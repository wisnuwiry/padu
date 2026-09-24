import { router } from "expo-router";
import type { ReactNode } from "react";
import { StyleSheet, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { IconButton } from "./button";
import { MaxContentWidth, Spacing } from "@/constants/theme";
import { useTheme } from "@/hooks/use-theme";

/** Pop when there is history; otherwise land on the task list. A screen
 * opened cold (deep link, state restore) is the stack's only entry, and a
 * bare router.back() there throws GO_BACK unhandled. */
export function navigateBack() {
  if (router.canGoBack()) router.back();
  else router.replace("/");
}

/**
 * The screen title bar: an opaque bar carrying a custom back button, the title
 * block, and an optional trailing action row. It sits in the layout above the
 * content rather than floating translucent over it.
 *
 * Drawn in JavaScript on purpose. The chrome it replaces was a Liquid Glass
 * material that only iOS 26 renders, so the title bar and its buttons looked
 * different on Android — and the translucent fallback stopped being legible
 * once content scrolled under it. Owning the bar keeps both platforms
 * identical, and an opaque one keeps the title readable with no blur pass.
 */
export function ScreenHeader({
  title,
  subtitle,
  right,
  back = true,
  leading,
}: {
  title?: string;
  subtitle?: string | null;
  right?: ReactNode;
  /** Off for a root screen, which has nothing to go back to. */
  back?: boolean;
  /** Replaces the title block — e.g. the daemon switcher on the task list. */
  leading?: ReactNode;
}) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  return (
    <View
      style={[
        styles.header,
        {
          backgroundColor: theme.background,
          borderBottomColor: theme.separator,
          paddingTop: insets.top,
        },
      ]}
    >
      <View style={styles.row}>
        {back ? (
          <IconButton
            glyphSize={21}
            icon="chevronLeft"
            label="Back"
            onPress={navigateBack}
          />
        ) : null}
        <View style={styles.titles}>
          {leading ?? (
            <>
              {title ? (
                <Text
                  numberOfLines={1}
                  style={[styles.title, { color: theme.text }]}
                >
                  {title}
                </Text>
              ) : null}
              {subtitle ? (
                <Text
                  numberOfLines={1}
                  style={[styles.subtitle, { color: theme.textTertiary }]}
                >
                  {subtitle}
                </Text>
              ) : null}
            </>
          )}
        </View>
        {right ? <View style={styles.right}>{right}</View> : null}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  header: { borderBottomWidth: StyleSheet.hairlineWidth },
  // Header content is capped and centred like the reading column, so a tablet
  // gets the same column as a phone with more margin.
  row: {
    alignItems: "center",
    alignSelf: "center",
    flexDirection: "row",
    gap: Spacing.one,
    maxWidth: MaxContentWidth,
    paddingBottom: Spacing.one,
    paddingHorizontal: Spacing.three,
    paddingTop: Spacing.one,
    width: "100%",
  },
  titles: {
    flex: 1,
    justifyContent: "center",
    minWidth: 0,
  },
  title: { fontSize: 17, fontWeight: "700", letterSpacing: -0.3 },
  subtitle: { fontSize: 12.5, marginTop: 1 },
  right: { alignItems: "center", flexDirection: "row", gap: Spacing.one },
});
