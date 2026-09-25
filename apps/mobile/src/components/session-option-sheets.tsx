import { BottomSheetFlatList, BottomSheetScrollView } from '@expo/ui/community/bottom-sheet';
import type { ProviderKind, ProviderModel, RuntimeMode } from '@padu/client';
import * as Haptics from 'expo-haptics';
import { useEffect, useMemo, useState } from 'react';
import {
  ActivityIndicator,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';

import { PaduIcon } from './padu-icon';
import { ProviderIcon } from './provider-icon';
import { ReasoningSlider } from './reasoning-slider';
import { Sheet, SheetRow } from './sheet';
import { NativeTint, Radius, Spacing } from '@/constants/theme';
import { useAllProviderModels, useProviderModels } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import { providerLabel, runtimeModeLabel } from '@/lib/session-presentation';

export interface ModelSelection {
  model: string | null;
  reasoningEffort: string | null;
}

export function modelDisplayName(
  models: ProviderModel[] | undefined,
  model: string | null,
): string {
  if (!model) return 'Default model';
  return models?.find((item) => item.id === model)?.name ?? model;
}


// ---------------------------------------------------------------------------
// ModelSheet — single-provider model + reasoning picker (from session screen)
// ---------------------------------------------------------------------------

/** Model + reasoning-effort picker, backed by the daemon's model discovery. */
export function ModelSheet({
  visible,
  onDismiss,
  provider,
  model,
  reasoningEffort,
  onApply,
}: {
  visible: boolean;
  onDismiss: () => void;
  provider: ProviderKind;
  model: string | null;
  reasoningEffort: string | null;
  onApply: (selection: ModelSelection) => void;
}) {
  const theme = useTheme();
  const probe = useProviderModels(visible ? provider : null);
  const models = probe.data?.models ?? [];
  const selected = model
    ? models.find((item) => item.id === model)
    : models.find((item) => item.is_default);
  const efforts = selected?.reasoning_efforts ?? [];
  const [localEffort, setLocalEffort] = useState<string | null>(reasoningEffort);

  useEffect(() => {
    if (visible) setLocalEffort(reasoningEffort);
  }, [visible, reasoningEffort]);

  function pickModel(next: ProviderModel) {
    void Haptics.selectionAsync();
    const effort = next.default_reasoning_effort ?? null;
    setLocalEffort(effort);
    onApply({ model: next.id, reasoningEffort: effort });
    if (!next.reasoning_efforts.length) onDismiss();
  }

  function pickEffort(id: string) {
    setLocalEffort(id);
    onApply({ model: selected?.id ?? model, reasoningEffort: id });
  }

  return (
    <Sheet onDismiss={onDismiss} scrollable={false} title={`${providerLabel(provider)} model`} visible={visible}>
      {probe.isPending ? (
        <View style={styles.loading}>
          <ActivityIndicator color={theme.textTertiary} />
        </View>
      ) : probe.error ? (
        <Text style={[styles.note, { color: theme.danger }]}>
          {probe.error instanceof Error ? probe.error.message : String(probe.error)}
        </Text>
      ) : (
        <View style={styles.modelSheetBody}>
          {/* Model list */}
          <BottomSheetScrollView
            alwaysBounceVertical={false}
            keyboardShouldPersistTaps="handled"
            showsVerticalScrollIndicator={false}
            style={styles.singleModelList}>
            {models.map((item) => (
              <SheetRow
                description={item.sub_provider ?? undefined}
                key={item.id}
                label={item.name}
                onPress={() => pickModel(item)}
                selected={model === item.id || (!model && item.is_default)}
              />
            ))}
            {!models.length && (
              <Text style={[styles.note, { color: theme.textTertiary }]}>
                This agent doesn't expose a model list; it will use its own default.
              </Text>
            )}
          </BottomSheetScrollView>

          {/* Reasoning effort */}
          {efforts.length > 0 && (
            <ReasoningSlider
              efforts={efforts}
              selected={localEffort ?? selected?.default_reasoning_effort ?? null}
              onSelect={pickEffort}
            />
          )}
        </View>
      )}
    </Sheet>
  );
}

// ---------------------------------------------------------------------------
// ModelPickerSheet — cross-provider split-pane picker (new-task screen)
// ---------------------------------------------------------------------------

export interface ProviderModelSelection {
  provider: ProviderKind;
  model: string | null;
  reasoningEffort: string | null;
}

/**
 * Split-pane cross-provider model picker:
 * - Left (narrow) column: provider list with icon
 * - Right (wide) column: search + model list for the selected provider
 * - Bottom bar: Codex-style reasoning effort segmented control
 *
 * Works identically on iOS and Android — no native platform pickers.
 */
export function ModelPickerSheet({
  visible,
  onDismiss,
  providers,
  provider,
  model,
  onApply,
}: {
  visible: boolean;
  onDismiss: () => void;
  providers: ProviderKind[];
  provider: ProviderKind | null;
  model: string | null;
  onApply: (selection: ProviderModelSelection) => void;
}) {
  const theme = useTheme();
  const catalog = useAllProviderModels(visible ? providers : []);
  const [activeProvider, setActiveProvider] = useState<ProviderKind | null>(
    provider ?? providers[0] ?? null,
  );
  const [search, setSearch] = useState('');
  const [pendingEffort, setPendingEffort] = useState<string | null>(null);
  const [pendingModel, setPendingModel] = useState<string | null>(model);

  useEffect(() => {
    if (!visible) return;
    const initial = provider ?? providers[0] ?? null;
    setActiveProvider(initial);
    setSearch('');
    setPendingModel(model);
    setPendingEffort(null);
  }, [visible, provider, providers, model]);

  // When the provider changes, reset the pending model/effort
  function switchProvider(id: ProviderKind) {
    void Haptics.selectionAsync();
    setActiveProvider(id);
    setSearch('');
    setPendingModel(null);
    setPendingEffort(null);
  }

  const entry = catalog.find((item) => item.id === activeProvider);
  const filteredModels = useMemo<ProviderModel[]>(() => {
    const query = search.trim().toLocaleLowerCase();
    const all = entry?.models ?? [];
    if (!query) return all;
    return all.filter((item) =>
      item.name.toLocaleLowerCase().includes(query) ||
      item.id.toLocaleLowerCase().includes(query) ||
      (item.sub_provider?.toLocaleLowerCase().includes(query) ?? false),
    );
  }, [entry?.models, search]);

  const selectedModel = pendingModel
    ? (entry?.models ?? []).find((m) => m.id === pendingModel)
    : (entry?.models ?? []).find((m) => m.is_default);
  const efforts = selectedModel?.reasoning_efforts ?? [];

  function pickModel(next: ProviderModel) {
    if (!activeProvider) return;
    void Haptics.selectionAsync();
    setPendingModel(next.id);
    const effort = next.default_reasoning_effort ?? null;
    setPendingEffort(effort);
    onApply({ provider: activeProvider, model: next.id, reasoningEffort: effort });
    if (!next.reasoning_efforts.length) onDismiss();
  }

  function pickEffort(id: string) {
    if (!activeProvider) return;
    setPendingEffort(id);
    onApply({ provider: activeProvider, model: pendingModel, reasoningEffort: id });
  }

  return (
    <Sheet onDismiss={onDismiss} scrollable={false} visible={visible}>
      <View style={styles.pickerBody}>
        {/* ── Left: provider column ── */}
        <View style={[styles.providerCol, { borderRightColor: theme.border }]}>
          <BottomSheetScrollView
            alwaysBounceVertical={false}
            showsVerticalScrollIndicator={false}>
            {providers.map((id) => {
              const isActive = id === activeProvider;
              return (
                <Pressable
                  accessibilityLabel={providerLabel(id)}
                  accessibilityRole="button"
                  accessibilityState={{ selected: isActive }}
                  key={id}
                  onPress={() => switchProvider(id)}
                  style={({ pressed }) => [
                    styles.providerItem,
                    isActive && { backgroundColor: theme.accentSoft },
                    { opacity: pressed && !isActive ? 0.6 : 1 },
                  ]}>
                  <ProviderIcon provider={id} size={20} />
                </Pressable>
              );
            })}
            {!providers.length && (
              <Text style={[styles.note, { color: theme.textTertiary }]}>
                No providers installed.
              </Text>
            )}
          </BottomSheetScrollView>
        </View>

        {/* ── Right: search + model list ── */}
        <View style={styles.modelCol}>
          {/* Search */}
          <View style={[styles.searchField, { backgroundColor: theme.overlayStrong }]}>
            <PaduIcon name="search" size={14} tintColor={theme.textTertiary} />
            <TextInput
              accessibilityLabel="Search models"
              autoCapitalize="none"
              autoCorrect={false}
              placeholder="Search"
              placeholderTextColor={theme.textTertiary}
              selectionColor={NativeTint}
              style={[styles.searchInput, { color: theme.text }]}
              value={search}
              onChangeText={setSearch}
            />
            {search.length > 0 && (
              <Pressable
                accessibilityLabel="Clear search"
                accessibilityRole="button"
                hitSlop={8}
                onPress={() => setSearch('')}
                style={({ pressed }) => ({ opacity: pressed ? 0.5 : 1 })}>
                <PaduIcon name="x" size={14} tintColor={theme.textTertiary} />
              </Pressable>
            )}
          </View>

          {/* Model list */}
          {entry?.isPending ? (
            <View style={styles.loading}>
              <ActivityIndicator color={theme.textTertiary} />
            </View>
          ) : (
            <BottomSheetFlatList
              data={filteredModels}
              initialNumToRender={14}
              keyExtractor={(item) => item.id}
              keyboardShouldPersistTaps="handled"
              renderItem={({ item }) => (
                <ModelRow
                  item={item}
                  selected={
                    provider === activeProvider &&
                    (pendingModel === item.id || (!pendingModel && item.is_default))
                  }
                  onPress={() => pickModel(item)}
                />
              )}
              showsVerticalScrollIndicator={false}
              style={styles.modelList}
              ListEmptyComponent={(
                <Text style={[styles.note, { color: theme.textTertiary }]}>
                  {search.trim()
                    ? 'No models match your search.'
                    : 'No model list available; the agent will use its default.'}
                </Text>
              )}
            />
          )}
        </View>
      </View>

      {/* ── Bottom: reasoning effort ── */}
      {efforts.length > 0 && (
        <ReasoningSlider
          efforts={efforts}
          selected={pendingEffort ?? selectedModel?.default_reasoning_effort ?? null}
          onSelect={pickEffort}
        />
      )}
    </Sheet>
  );
}

/** Compact model row for the right-pane list. */
function ModelRow({
  item,
  selected,
  onPress,
}: {
  item: ProviderModel;
  selected: boolean;
  onPress: () => void;
}) {
  const theme = useTheme();
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ selected }}
      onPress={onPress}
      style={({ pressed }) => [
        styles.modelRow,
        {
          backgroundColor: pressed
            ? theme.overlayStrong
            : selected
              ? theme.overlay
              : 'transparent',
        },
      ]}>
      <View style={styles.modelRowCopy}>
        <Text numberOfLines={1} style={[styles.modelRowLabel, { color: theme.text }]}>
          {item.name}
        </Text>
        {item.sub_provider ? (
          <Text numberOfLines={1} style={[styles.modelRowSub, { color: theme.textTertiary }]}>
            {item.sub_provider}
          </Text>
        ) : null}
      </View>
      {selected && (
        <PaduIcon name="check" size={14} tintColor={theme.accent} />
      )}
    </Pressable>
  );
}

// ---------------------------------------------------------------------------
// AccessSheet
// ---------------------------------------------------------------------------

const ACCESS_MODES: Array<{ id: RuntimeMode; description: string }> = [
  { id: 'ask', description: 'Approve every command and file edit.' },
  { id: 'autoAcceptEdits', description: 'Edits apply automatically; commands still ask.' },
  { id: 'auto', description: 'Works autonomously inside the project.' },
  { id: 'fullAccess', description: 'No approval prompts. The agent acts freely.' },
];

/** Access-mode picker mirroring the desktop composer's access control. */
export function AccessSheet({
  visible,
  onDismiss,
  mode,
  onApply,
}: {
  visible: boolean;
  onDismiss: () => void;
  mode: RuntimeMode;
  onApply: (mode: RuntimeMode) => void;
}) {
  return (
    <Sheet onDismiss={onDismiss} title="Agent access" visible={visible}>
      {ACCESS_MODES.map((item) => (
        <SheetRow
          description={item.description}
          key={item.id}
          label={runtimeModeLabel(item.id)}
          onPress={() => {
            void Haptics.selectionAsync();
            onApply(item.id);
            onDismiss();
          }}
          selected={mode === item.id}
        />
      ))}
    </Sheet>
  );
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const PROVIDER_COL_WIDTH = 56;

const styles = StyleSheet.create({
  // shared
  loading: { alignItems: 'center', justifyContent: 'center', paddingVertical: 40 },
  note: { fontSize: 13, lineHeight: 18, paddingHorizontal: 12, paddingVertical: 10 },

  // ModelSheet (single provider)
  modelSheetBody: { minHeight: 180 },
  singleModelList: { maxHeight: 320 },

  // ModelPickerSheet split-pane
  pickerBody: {
    flexDirection: 'row',
    minHeight: 300,
  },
  providerCol: {
    borderRightWidth: StyleSheet.hairlineWidth,
    paddingTop: 6,
    width: PROVIDER_COL_WIDTH,
  },
  providerItem: {
    alignItems: 'center',
    borderRadius: Radius.medium,
    justifyContent: 'center',
    marginHorizontal: 6,
    marginVertical: 2,
    height: 40,
  },
  // no-op kept for possible future use
  providerItemActive: {},
  providerItemLabel: {},
  providerActiveBar: {},

  // Right model column
  modelCol: { flex: 1, paddingTop: 4 },
  searchField: {
    alignItems: 'center',
    borderRadius: Radius.medium,
    flexDirection: 'row',
    gap: 7,
    marginBottom: 4,
    marginHorizontal: Spacing.two,
    minHeight: 34,
    paddingHorizontal: 10,
  },
  searchInput: { flex: 1, fontSize: 14, paddingVertical: 5 },
  modelList: { maxHeight: 260 },
  modelRow: {
    alignItems: 'center',
    borderRadius: Radius.small,
    flexDirection: 'row',
    gap: 8,
    marginHorizontal: Spacing.two,
    marginVertical: 1,
    minHeight: 40,
    paddingHorizontal: 10,
    paddingVertical: 5,
  },
  modelRowCopy: { flex: 1, minWidth: 0 },
  modelRowLabel: { fontSize: 13, fontWeight: '500' },
  modelRowSub: { fontSize: 11, lineHeight: 14, marginTop: 1 },

  // legacy
  sectionTitle: {
    fontSize: 12,
    fontWeight: '600',
    letterSpacing: 0.4,
    marginBottom: 6,
    marginHorizontal: 12,
    marginTop: 14,
  },
});
