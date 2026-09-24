import { BottomSheetFlatList } from '@expo/ui/community/bottom-sheet';
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
  useWindowDimensions,
} from 'react-native';
import Animated, {
  Easing,
  useAnimatedStyle,
  useReducedMotion,
  useSharedValue,
  withTiming,
} from 'react-native-reanimated';

import { PaduIcon } from './padu-icon';
import { ProviderIcon } from './provider-icon';
import { Sheet, SheetRow } from './sheet';
import { NativeTint, Radius } from '@/constants/theme';
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
  const selected = model ? models.find((item) => item.id === model) : models.find((item) => item.is_default);
  const efforts = selected?.reasoning_efforts ?? [];

  function pickModel(next: ProviderModel) {
    void Haptics.selectionAsync();
    onApply({
      model: next.id,
      reasoningEffort: next.default_reasoning_effort ?? null,
    });
    if (!next.reasoning_efforts.length) onDismiss();
  }

  return (
    <Sheet onDismiss={onDismiss} title={`${providerLabel(provider)} model`} visible={visible}>
      {probe.isPending ? (
        <View style={styles.loading}>
          <ActivityIndicator color={theme.textTertiary} />
        </View>
      ) : probe.error ? (
        <Text style={[styles.note, { color: theme.danger }]}>
          {probe.error instanceof Error ? probe.error.message : String(probe.error)}
        </Text>
      ) : (
        <>
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
              This agent doesn’t expose a model list; it will use its own default.
            </Text>
          )}
          {efforts.length ? (
            <>
              <Text style={[styles.sectionTitle, { color: theme.textSecondary }]}>
                REASONING EFFORT
              </Text>
              {efforts.map((effort) => (
                <SheetRow
                  description={effort.description ?? undefined}
                  key={effort.id}
                  label={effort.label}
                  onPress={() => {
                    void Haptics.selectionAsync();
                    onApply({ model: selected?.id ?? model, reasoningEffort: effort.id });
                    onDismiss();
                  }}
                  selected={(reasoningEffort ?? selected?.default_reasoning_effort) === effort.id}
                />
              ))}
            </>
          ) : null}
        </>
      )}
    </Sheet>
  );
}

export interface ProviderModelSelection {
  provider: ProviderKind;
  model: string | null;
  reasoningEffort: string | null;
}


/**
 * Two-screen cross-provider model picker: opening lands on the current
 * provider's models with a search filter over a virtualized list; the back
 * row (named after the provider) slides across to the providers screen, and
 * choosing a provider slides back into that provider's models.
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
  const { height: windowHeight } = useWindowDimensions();
  const catalog = useAllProviderModels(visible ? providers : []);
  const [browsing, setBrowsing] = useState<ProviderKind | null>(provider);
  const [search, setSearch] = useState('');
  const reduceMotion = useReducedMotion();
  // 0 = providers page, 1 = models page.
  const progress = useSharedValue(1);
  const pageWidth = useSharedValue(0);
  const slideStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: -pageWidth.value * progress.value }],
  }));

  useEffect(() => {
    if (!visible) return;
    const initial = provider ?? providers[0] ?? null;
    setBrowsing(initial);
    setSearch('');
    progress.value = initial ? 1 : 0;
  }, [progress, provider, providers, visible]);

  function slideTo(target: 0 | 1) {
    progress.value = reduceMotion
      ? target
      : withTiming(target, { duration: 260, easing: Easing.bezier(0.32, 0.72, 0.25, 1) });
  }

  const entry = catalog.find((item) => item.id === browsing);
  const listHeight = Math.round(windowHeight * 0.48);
  const items = useMemo<ProviderModel[]>(() => {
    const query = search.trim().toLocaleLowerCase();
    const models = entry?.models ?? [];
    if (!query) return models;
    return models.filter((item) => (
      item.name.toLocaleLowerCase().includes(query) ||
        item.id.toLocaleLowerCase().includes(query) ||
        item.sub_provider?.toLocaleLowerCase().includes(query)
    ));
  }, [entry?.models, search]);

  function pickModel(next: ProviderModel) {
    if (!browsing) return;
    void Haptics.selectionAsync();
    onApply({
      provider: browsing,
      model: next.id,
      reasoningEffort: next.default_reasoning_effort ?? null,
    });
    onDismiss();
  }

  return (
    <Sheet onDismiss={onDismiss} scrollable={false} visible={visible}>
      <View
        style={styles.pagerClip}
        onLayout={(event) => {
          pageWidth.value = event.nativeEvent.layout.width;
        }}>
        <Animated.View style={[styles.pagerTrack, slideStyle]}>
          <View style={styles.page}>
            <BottomSheetFlatList
              data={providers}
              keyExtractor={(id) => id}
              keyboardShouldPersistTaps="handled"
              renderItem={({ item: id }) => (
                <SheetRow
                  label={providerLabel(id)}
                  leading={<ProviderIcon provider={id} size={20} />}
                  onPress={() => {
                    void Haptics.selectionAsync();
                    setBrowsing(id);
                    setSearch('');
                    slideTo(1);
                  }}
                  selected={id === provider}
                />
              )}
              showsVerticalScrollIndicator={false}
              style={{ height: listHeight }}
              ListEmptyComponent={(
                <Text style={[styles.note, { color: theme.textTertiary }]}>
                  No agents are installed on this daemon host.
                </Text>
              )}
            />
          </View>
          <View style={styles.page}>
            <Pressable
              accessibilityHint="Shows all providers"
              accessibilityLabel={browsing ? providerLabel(browsing) : 'Provider'}
              accessibilityRole="button"
              onPress={() => slideTo(0)}
              style={({ pressed }) => [styles.backRow, { opacity: pressed ? 0.55 : 1 }]}>
              <PaduIcon
                name="chevronLeft"
                size={14}
                tintColor={theme.accent}
              />
              <Text style={[styles.backLabel, { color: theme.accent }]}>
                {browsing ? providerLabel(browsing) : 'Provider'}
              </Text>
            </Pressable>
            <View style={[styles.searchField, { backgroundColor: theme.overlayStrong }]}>
              <PaduIcon
                name="search"
                size={14}
                tintColor={theme.textTertiary}
              />
              <TextInput
                accessibilityLabel="Search models"
                autoCapitalize="none"
                autoCorrect={false}
                placeholder="Search models"
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
                  <PaduIcon
                    name="x"
                    size={15}
                    tintColor={theme.textTertiary}
                  />
                </Pressable>
              )}
            </View>
            {entry?.isPending ? (
              <View style={[styles.loading, { height: listHeight }]}>
                <ActivityIndicator color={theme.textTertiary} />
              </View>
            ) : (
              <BottomSheetFlatList
                data={items}
                initialNumToRender={14}
                keyExtractor={(item) => item.id}
                keyboardShouldPersistTaps="handled"
                renderItem={({ item }) => (
                  <SheetRow
                    description={item.sub_provider ?? undefined}
                    label={item.name}
                    onPress={() => pickModel(item)}
                    selected={provider === browsing &&
                      (model === item.id || (!model && item.is_default))}
                  />
                )}
                showsVerticalScrollIndicator={false}
                style={{ height: listHeight }}
                ListEmptyComponent={(
                  <Text style={[styles.note, { color: theme.textTertiary }]}>
                    {search.trim()
                      ? 'No models match your search.'
                      : 'This agent doesn’t expose a model list; it will use its own default.'}
                  </Text>
                )}
              />
            )}
          </View>
        </Animated.View>
      </View>
    </Sheet>
  );
}

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

const styles = StyleSheet.create({
  loading: { alignItems: 'center', justifyContent: 'center', paddingVertical: 40 },
  note: { fontSize: 13, lineHeight: 18, paddingHorizontal: 12, paddingVertical: 10 },
  pagerClip: { overflow: 'hidden' },
  pagerTrack: { flexDirection: 'row', width: '200%' },
  page: { paddingTop: 4, width: '50%' },
  backRow: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 5,
    minHeight: 40,
    paddingHorizontal: 10,
  },
  backLabel: { fontSize: 15, fontWeight: '600' },
  searchField: {
    alignItems: 'center',
    borderRadius: Radius.medium,
    flexDirection: 'row',
    gap: 7,
    marginBottom: 8,
    marginHorizontal: 4,
    minHeight: 38,
    paddingHorizontal: 10,
  },
  searchInput: { flex: 1, fontSize: 15, paddingVertical: 7 },
  sectionTitle: {
    fontSize: 12,
    fontWeight: '600',
    letterSpacing: 0.4,
    marginBottom: 6,
    marginHorizontal: 12,
    marginTop: 14,
  },
});
