import * as Haptics from "expo-haptics";
import { Stack, useLocalSearchParams } from "expo-router";
import { navigateBack } from "@/components/screen-header";
import { useMemo, useRef, useState } from "react";
import {
  ActivityIndicator,
  Alert,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
  type TextInputInstance,
} from "react-native";

import { PaduIcon } from "@/components/padu-icon";
import { ConnectionNote } from "@/components/connection-note";
import { TransportBadge } from "@/components/transport-badge";
import { MaxContentWidth, Radius, Spacing } from "@/constants/theme";
import { useTheme } from "@/hooks/use-theme";
import { useDaemon } from "@/lib/daemon-context";
import {
  classifyConnection,
  describeConnection,
  displayHost,
  type ConnectionClass,
  type DaemonTransport,
} from "@/lib/daemon-profile";

export default function DaemonEditorScreen() {
  const theme = useTheme();
  const params = useLocalSearchParams<{ id?: string | string[] }>();
  const profileId = Array.isArray(params.id) ? params.id[0] : params.id;
  const daemon = useDaemon();
  const profile = daemon.profiles.find((item) => item.id === profileId);
  const [name, setName] = useState(profile?.name ?? "");
  const [address, setAddress] = useState(profile?.address ?? "");
  const [token, setToken] = useState("");
  const [revealed, setRevealed] = useState(false);
  const [saving, setSaving] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const addressInput = useRef<TextInputInstance>(null);
  const tokenInput = useRef<TextInputInstance>(null);

  const connectionClass = useMemo(() => {
    if (!address.trim()) return null;
    return classifyConnection(address);
  }, [address]);
  // A plaintext address to a public host is refused by `saveProfile`, so the
  // action is disabled here rather than left to fail on submit.
  const canSave = Boolean(
    address.trim() &&
    (profile || token.trim()) &&
    connectionClass !== "invalid" &&
    connectionClass !== "insecure_public_ws" &&
    !saving &&
    !removing,
  );

  async function save() {
    if (!canSave) return;
    setSaving(true);
    setLocalError(null);
    try {
      const result = await daemon.saveProfile(
        { name, address, token },
        profile?.id,
      );
      await Haptics.notificationAsync(
        result.connected
          ? Haptics.NotificationFeedbackType.Success
          : Haptics.NotificationFeedbackType.Warning,
      );
      navigateBack();
    } catch (cause) {
      setLocalError(cause instanceof Error ? cause.message : String(cause));
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
    } finally {
      setSaving(false);
    }
  }

  async function remove() {
    if (!profile || removing || saving) return;
    setRemoving(true);
    setLocalError(null);
    try {
      await daemon.removeProfile(profile.id);
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);
      navigateBack();
    } catch (cause) {
      setLocalError(cause instanceof Error ? cause.message : String(cause));
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
    } finally {
      setRemoving(false);
    }
  }

  function confirmRemove() {
    if (!profile || removing || saving) return;
    Alert.alert(
      `Remove ${profile.name}?`,
      "This removes the saved address and token from this device. Tasks remain on the daemon host.",
      [
        { text: "Cancel", style: "cancel" },
        {
          text: "Remove",
          style: "destructive",
          onPress: () => void remove(),
        },
      ],
    );
  }

  return (
    <View style={[styles.screen, { backgroundColor: theme.background }]}>
      <Stack.Screen
        options={{
          title: profile ? "Edit Daemon" : "Add Daemon",
          contentStyle: { backgroundColor: theme.background },
          headerBackVisible: false,
          headerStyle: { backgroundColor: theme.background },
          headerTintColor: theme.accent,
          headerTitleStyle: { color: theme.text },
          gestureEnabled: !saving && !removing,
          ...(Platform.OS === "ios"
            ? {
                unstable_headerLeftItems: () => [
                  {
                    type: "button" as const,
                    label: "Cancel",
                    disabled: saving || removing,
                    onPress: navigateBack,
                  },
                ],
                unstable_headerRightItems: () => [
                  {
                    type: "button" as const,
                    label: saving ? "Saving…" : profile ? "Save" : "Add",
                    variant: "done" as const,
                    disabled: !canSave,
                    onPress: () => void save(),
                  },
                ],
              }
            : {
                headerLeft: () => (
                  <HeaderButton
                    disabled={saving || removing}
                    label="Cancel"
                    onPress={navigateBack}
                  />
                ),
                headerRight: () => (
                  <HeaderButton
                    disabled={!canSave}
                    emphasized
                    label={saving ? "Saving…" : profile ? "Save" : "Add"}
                    onPress={() => void save()}
                  />
                ),
              }),
        }}
      />
      <ScrollView
        automaticallyAdjustKeyboardInsets
        contentContainerStyle={styles.content}
        contentInsetAdjustmentBehavior="automatic"
        keyboardDismissMode="interactive"
        keyboardShouldPersistTaps="handled"
        showsVerticalScrollIndicator={false}>
        <View style={styles.column}>
          <Text style={[styles.sectionLabel, { color: theme.textSecondary }]}>
            CONNECTION
          </Text>
          <View style={[styles.formGroup, { backgroundColor: theme.surface }]}>
            <View style={styles.formRow}>
              <Text style={[styles.fieldLabel, { color: theme.text }]}>
                Name
              </Text>
              <TextInput
                accessibilityLabel="Daemon name"
                autoCapitalize="words"
                autoCorrect={false}
                editable={!saving && !removing}
                onChangeText={(value) => {
                  setName(value);
                  setLocalError(null);
                }}
                onSubmitEditing={() => addressInput.current?.focus()}
                placeholder="Optional"
                placeholderTextColor={theme.textTertiary}
                returnKeyType="next"
                selectionColor={theme.accent}
                style={[styles.rowInput, { color: theme.text }]}
                value={name}
              />
            </View>

            <View
              style={[styles.separator, { backgroundColor: theme.separator }]}
            />
            <View style={styles.formRow}>
              <Text style={[styles.fieldLabel, { color: theme.text }]}>
                Address
              </Text>
              <TextInput
                ref={addressInput}
                accessibilityLabel="Daemon WebSocket address"
                autoCapitalize="none"
                autoCorrect={false}
                clearButtonMode="while-editing"
                editable={!saving && !removing}
                inputMode="url"
                keyboardType="url"
                onChangeText={(value) => {
                  setAddress(value);
                  setLocalError(null);
                }}
                onSubmitEditing={() => tokenInput.current?.focus()}
                placeholder="wss://host.example"
                placeholderTextColor={theme.textTertiary}
                returnKeyType="next"
                selectionColor={theme.accent}
                spellCheck={false}
                style={[styles.rowInput, { color: theme.text }]}
                value={address}
              />
            </View>

            <View
              style={[styles.separator, { backgroundColor: theme.separator }]}
            />
            <View style={styles.formRow}>
              <Text style={[styles.fieldLabel, { color: theme.text }]}>
                Token
              </Text>
              <TextInput
                ref={tokenInput}
                accessibilityLabel="Daemon token"
                autoCapitalize="none"
                autoComplete="off"
                autoCorrect={false}
                editable={!saving && !removing}
                onChangeText={(value) => {
                  setToken(value);
                  setLocalError(null);
                }}
                placeholder={profile ? "Unchanged" : "Required"}
                placeholderTextColor={theme.textTertiary}
                returnKeyType="done"
                secureTextEntry={!revealed}
                selectionColor={theme.accent}
                spellCheck={false}
                style={[
                  styles.rowInput,
                  styles.tokenText,
                  { color: theme.text },
                ]}
                value={token}
                onSubmitEditing={() => void save()}
              />
              <Pressable
                accessibilityLabel={revealed ? "Hide token" : "Reveal token"}
                accessibilityRole="button"
                accessibilityState={{ selected: revealed }}
                disabled={saving || removing}
                onPress={() => setRevealed((value) => !value)}
                style={({ pressed }) => [
                  styles.revealButton,
                  { opacity: pressed ? 0.45 : 1 },
                ]}>
                <PaduIcon
                  name={revealed ? "eyeOff" : "eye"}
                  size={18}
                  tintColor={theme.textSecondary}
                />
              </Pressable>
            </View>
          </View>

          <View style={styles.previewSlot}>
            <ConnectionPreview
              address={address}
              connectionClass={connectionClass}
              profile={Boolean(profile)}
            />
          </View>

          {localError ? (
            <View accessibilityLiveRegion="polite" style={styles.messageRow}>
              <PaduIcon
                name="alert"
                size={14}
                tintColor={theme.danger}
              />
              <Text style={[styles.messageText, { color: theme.danger }]}>
                {localError}
              </Text>
            </View>
          ) : null}

          {profile ? (
            <>
              <Text
                style={[
                  styles.sectionLabel,
                  styles.actionsLabel,
                  { color: theme.textSecondary },
                ]}>
                ACTIONS
              </Text>
              <View
                style={[styles.formGroup, { backgroundColor: theme.surface }]}>
                <Pressable
                  accessibilityRole="button"
                  disabled={removing || saving}
                  onPress={confirmRemove}
                  style={({ pressed }) => [
                    styles.removeRow,
                    { opacity: removing || saving || pressed ? 0.45 : 1 },
                  ]}>
                  {removing && (
                    <ActivityIndicator color={theme.danger} size="small" />
                  )}
                  <Text style={[styles.removeText, { color: theme.danger }]}>
                    {removing ? "Removing…" : "Remove Daemon"}
                  </Text>
                </Pressable>
              </View>
              <Text
                style={[styles.actionFootnote, { color: theme.textSecondary }]}>
                Removes the saved address and token from this device. Tasks
                remain on the daemon host.
              </Text>
            </>
          ) : null}
        </View>
      </ScrollView>
    </View>
  );
}

/**
 * What the typed address currently resolves to: the host that will be dialled,
 * its transport, and how it is protected. Falls back to guidance while the
 * field is empty, and to the parse error when the address cannot be read.
 */
function ConnectionPreview({
  address,
  connectionClass,
  profile,
}: {
  address: string;
  connectionClass: ConnectionClass | null;
  profile: boolean;
}) {
  const theme = useTheme();

  if (!address.trim()) {
    return (
      <ConnectionNote
        tone="neutral"
        text={
          profile
            ? "Leave the token blank to keep the saved credential."
            : "Copy the address and token from Padu Desktop → Settings → Daemon."
        }
      />
    );
  }

  if (!connectionClass || connectionClass === "invalid") {
    return (
      <ConnectionNote tone="danger" text={describeConnection("invalid").text} />
    );
  }

  const description = describeConnection(connectionClass);
  // Where the credential ends up is the other half of trusting a host, and it
  // differs per surface the app runs on.
  const text =
    description.tone === "secure"
      ? `${description.text} ${
          Platform.select({
            web: "The token stays in this browser.",
            default: "The token is stored in this device’s keychain.",
          }) ?? ""
        }`
      : description.text;

  return (
    <View
      style={[
        styles.preview,
        { backgroundColor: theme.surface, borderColor: theme.border },
      ]}>
      <View style={styles.previewHeader}>
        <Text
          numberOfLines={1}
          style={[styles.previewHost, { color: theme.text }]}>
          {displayHost(address)}
        </Text>
        <TransportBadge kind={transportForClass(connectionClass)} />
      </View>
      <ConnectionNote tone={description.tone} text={text} />
    </View>
  );
}

/** The transport a classified address implies. */
function transportForClass(connectionClass: ConnectionClass): DaemonTransport {
  return connectionClass === "cloudflare" ||
    connectionClass === "tailscale" ||
    connectionClass === "ssh_relay"
    ? connectionClass
    : "direct";
}

function HeaderButton({
  disabled,
  emphasized = false,
  label,
  onPress,
}: {
  disabled: boolean;
  emphasized?: boolean;
  label: string;
  onPress: () => void;
}) {
  const theme = useTheme();
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ disabled }}
      disabled={disabled}
      hitSlop={8}
      onPress={onPress}
      style={({ pressed }) => [
        styles.headerButton,
        { opacity: disabled ? 0.35 : pressed ? 0.5 : 1 },
      ]}>
      <Text
        style={[
          styles.headerButtonText,
          emphasized && styles.headerButtonEmphasized,
          { color: theme.accent },
        ]}>
        {label}
      </Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1 },
  content: {
    paddingBottom: Spacing.five,
    paddingHorizontal: Spacing.three,
    paddingTop: Spacing.four,
  },
  column: { alignSelf: "center", maxWidth: MaxContentWidth, width: "100%" },
  sectionLabel: {
    fontSize: 13,
    fontWeight: "500",
    letterSpacing: 0.1,
    marginBottom: Spacing.two,
    paddingHorizontal: Spacing.one,
  },
  actionsLabel: { marginTop: Spacing.four },
  formGroup: { borderRadius: Radius.medium, overflow: "hidden" },
  formRow: {
    alignItems: "center",
    flexDirection: "row",
    minHeight: 56,
    paddingLeft: Spacing.three,
    paddingRight: Spacing.two,
  },
  fieldLabel: { fontSize: 16, width: 82 },
  rowInput: {
    flex: 1,
    fontSize: 16,
    minHeight: 55,
    paddingHorizontal: Spacing.two,
    paddingVertical: Spacing.two,
    textAlign: Platform.select({ ios: "right", default: "left" }),
  },
  tokenText: { paddingRight: Spacing.half },
  revealButton: {
    alignItems: "center",
    height: 44,
    justifyContent: "center",
    width: 44,
  },
  separator: { height: StyleSheet.hairlineWidth, marginLeft: Spacing.three },
  previewSlot: { marginTop: Spacing.three },
  preview: {
    borderRadius: Radius.large,
    borderWidth: StyleSheet.hairlineWidth,
    gap: Spacing.two,
    padding: Spacing.three,
  },
  previewHeader: {
    alignItems: "center",
    flexDirection: "row",
    gap: Spacing.two,
  },
  previewHost: { flex: 1, fontSize: 15.5, fontWeight: "600" },
  messageRow: {
    alignItems: "flex-start",
    flexDirection: "row",
    gap: Spacing.two,
    marginTop: Spacing.two,
  },
  messageText: { flex: 1, fontSize: 13, lineHeight: 18 },
  removeRow: {
    alignItems: "center",
    flexDirection: "row",
    gap: Spacing.two,
    justifyContent: "center",
    minHeight: 52,
    paddingHorizontal: Spacing.three,
  },
  removeText: { fontSize: 16 },
  actionFootnote: { fontSize: 13, lineHeight: 18, marginTop: Spacing.two },
  headerButton: { justifyContent: "center", minHeight: 44, minWidth: 44 },
  headerButtonText: { fontSize: 16 },
  headerButtonEmphasized: { fontWeight: "700" },
});
