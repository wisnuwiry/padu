import { router } from 'expo-router';
import {
  Image,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  View,
  useWindowDimensions,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { PaduIcon, type PaduIconName } from '@/components/padu-icon';
import { ThemeChoiceControl } from '@/components/theme-choice-control';
import { MaxContentWidth, Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';

/**
 * The first-run screen, shown while no daemon is saved: what Padu is, the theme
 * choice, and the two ways to add a host.
 */
export function Onboarding() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const { width } = useWindowDimensions();
  // Roomier gutters once there is width for them, without switching layout.
  const wide = width >= 640;
  return (
    <ScrollView
      contentContainerStyle={[
        styles.content,
        {
          paddingBottom: insets.bottom + Spacing.five,
          paddingTop: insets.top + Spacing.four,
        },
      ]}
      showsVerticalScrollIndicator={false}>
      <View
        style={[
          styles.column,
          { paddingHorizontal: wide ? Spacing.five : Spacing.four },
        ]}>
        <Image
          accessibilityLabel="Padu"
          source={require('@/assets/images/icon.png')}
          style={styles.appIcon}
        />
        <Text style={[styles.title, { color: theme.text }]}>Your agents, everywhere.</Text>
        <Text style={[styles.body, { color: theme.textSecondary }]}>
          Connect to Padu running on your computer, workstation, or private server. Add more than
          one and switch whenever you need.
        </Text>

        <View style={styles.cards}>
          <OnboardingHighlight
            description="Padu lives on your computer, workstation, or private server — the phone just drives it."
            icon="laptop"
            title="Runs on your machine"
          />
          <OnboardingHighlight
            description="Save several hosts and move between them without re-entering credentials."
            icon="server"
            title="Switch anytime"
          />
          <OnboardingHighlight
            description="Tokens stay in this device’s keychain and go straight to the host you choose."
            icon="lock"
            title="Private by default"
          />
        </View>

        <View style={styles.themeChoice}>
          <ThemeChoiceControl />
        </View>

        <View style={styles.actions}>
          <Pressable
            accessibilityHint="Opens the daemon editor"
            accessibilityLabel="Add a daemon"
            accessibilityRole="button"
            onPress={() => router.push('/daemon-editor')}
            style={({ pressed }) => [
              styles.primaryButton,
              { backgroundColor: theme.inverse, opacity: pressed ? 0.78 : 1 },
            ]}>
            <PaduIcon
              name="plus"
              size={17}
              tintColor={theme.onInverse}
            />
            <Text style={[styles.primaryButtonText, { color: theme.onInverse }]}>Add a daemon</Text>
          </Pressable>
          <Pressable
            accessibilityHint="Import a daemon from the desktop’s QR code"
            accessibilityLabel="Import from link"
            accessibilityRole="button"
            onPress={() => router.push('/daemon-import')}
            style={({ pressed }) => [
              styles.secondaryButton,
              {
                backgroundColor: pressed ? theme.overlayStrong : theme.surface,
                borderColor: theme.borderStrong,
              },
            ]}>
            <PaduIcon
              name="scan"
              size={16}
              tintColor={theme.textSecondary}
            />
            <Text style={[styles.secondaryButtonText, { color: theme.text }]}>Import from link</Text>
          </Pressable>
        </View>
      </View>
    </ScrollView>
  );
}

function OnboardingHighlight({
  description,
  icon,
  title,
}: {
  description: string;
  icon: PaduIconName;
  title: string;
}) {
  const theme = useTheme();
  return (
    <View style={[styles.highlight, { backgroundColor: theme.raised, borderColor: theme.border }]}>
      <View
        style={[
          styles.highlightIcon,
          { backgroundColor: theme.background, borderColor: theme.borderStrong },
        ]}>
        <PaduIcon name={icon} size={15} tintColor={theme.textSecondary} />
      </View>
      <View style={styles.highlightCopy}>
        <Text style={[styles.highlightTitle, { color: theme.text }]}>{title}</Text>
        <Text style={[styles.highlightBody, { color: theme.textSecondary }]}>{description}</Text>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  content: { alignItems: 'center', flexGrow: 1, justifyContent: 'center' },
  column: { alignItems: 'center', maxWidth: MaxContentWidth, width: '100%' },
  appIcon: { borderRadius: Radius.large, height: 72, marginBottom: Spacing.four, width: 72 },
  title: { fontSize: 28, fontWeight: '700', letterSpacing: -0.7, textAlign: 'center' },
  body: {
    fontSize: 15,
    lineHeight: 21,
    marginTop: Spacing.two,
    maxWidth: 360,
    textAlign: 'center',
  },
  cards: { gap: Spacing.two, marginTop: Spacing.five, maxWidth: 420, width: '100%' },
  highlight: {
    alignItems: 'flex-start',
    borderRadius: Radius.medium,
    borderWidth: StyleSheet.hairlineWidth,
    flexDirection: 'row',
    gap: 12,
    padding: 12,
  },
  highlightIcon: {
    alignItems: 'center',
    borderRadius: Radius.small,
    borderWidth: StyleSheet.hairlineWidth,
    height: 30,
    justifyContent: 'center',
    width: 30,
  },
  highlightCopy: { flex: 1, gap: Spacing.half, minWidth: 0 },
  highlightTitle: { fontSize: 15, fontWeight: '600' },
  highlightBody: { fontSize: 12.5, lineHeight: 17 },
  themeChoice: { marginTop: Spacing.four, maxWidth: 420, width: '100%' },
  actions: { gap: Spacing.two, marginTop: Spacing.five, maxWidth: 420, width: '100%' },
  primaryButton: {
    alignItems: 'center',
    borderRadius: Radius.large,
    flexDirection: 'row',
    gap: Spacing.two,
    justifyContent: 'center',
    minHeight: 50,
  },
  primaryButtonText: { fontSize: 15, fontWeight: '700' },
  secondaryButton: {
    alignItems: 'center',
    borderRadius: Radius.large,
    borderWidth: 1.5,
    flexDirection: 'row',
    gap: Spacing.two,
    justifyContent: 'center',
    minHeight: 50,
  },
  secondaryButtonText: { fontSize: 15, fontWeight: '600' },
});
