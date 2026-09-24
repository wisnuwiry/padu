import { Fragment, memo, useMemo } from 'react';
import { StyleSheet, View } from 'react-native';
import { useMarkdown, type MarkedStyles } from 'react-native-marked';

import { MonoFont, Radius } from '@/constants/theme';
import { useColorScheme } from '@/hooks/use-color-scheme';
import { useTheme } from '@/hooks/use-theme';

/**
 * Markdown body for assistant responses and expanded reasoning. Memoized on
 * the raw string so a streaming transcript only re-parses the message that
 * changed.
 */
export const MarkdownMessage = memo(function MarkdownMessage({
  value,
  compact = false,
}: {
  value: string;
  compact?: boolean;
}) {
  const theme = useTheme();
  const colorScheme = useColorScheme();
  const bodyColor = compact ? theme.textSecondary : theme.text;
  const bodySize = compact ? 13.5 : 15.5;
  const bodyLineHeight = compact ? 20 : 23;
  const styles = useMemo<MarkedStyles>(() => ({
    text: { color: bodyColor, fontSize: bodySize, lineHeight: bodyLineHeight },
    paragraph: { paddingVertical: 4 },
    h1: { borderBottomWidth: 0, color: theme.text, fontSize: 22, fontWeight: '700', lineHeight: 29, marginVertical: 9, paddingBottom: 0 },
    h2: { borderBottomWidth: 0, color: theme.text, fontSize: 19, fontWeight: '700', lineHeight: 26, marginVertical: 8, paddingBottom: 0 },
    h3: { color: theme.text, fontSize: 17, fontWeight: '600', lineHeight: 24, marginVertical: 7 },
    h4: { color: theme.text, fontSize: 15.5, fontWeight: '600', lineHeight: 22, marginVertical: 6 },
    h5: { color: theme.text, fontSize: 14.5, fontWeight: '600', lineHeight: 21, marginVertical: 5 },
    h6: { color: theme.textSecondary, fontSize: 13, fontWeight: '600', lineHeight: 19, marginVertical: 5 },
    strong: { color: bodyColor, fontSize: bodySize, fontWeight: '600', lineHeight: bodyLineHeight },
    em: { color: bodyColor, fontSize: bodySize, lineHeight: bodyLineHeight },
    strikethrough: { color: theme.textSecondary, fontSize: bodySize, lineHeight: bodyLineHeight },
    link: { color: theme.accent, fontStyle: 'normal', textDecorationLine: 'underline' },
    blockquote: { borderLeftColor: theme.borderStrong, borderLeftWidth: 3, marginVertical: 5, opacity: 0.9, paddingLeft: 11 },
    codespan: {
      backgroundColor: theme.codeWash,
      borderRadius: 4,
      color: theme.codeText,
      fontFamily: MonoFont,
      fontSize: compact ? 12 : 13,
      fontStyle: 'normal',
      fontWeight: '400',
    },
    code: {
      backgroundColor: theme.inset,
      borderColor: theme.border,
      borderRadius: Radius.small,
      borderWidth: StyleSheet.hairlineWidth,
      marginVertical: 6,
      padding: 11,
    },
    hr: { borderBottomColor: theme.separator, marginVertical: 12 },
    list: { marginVertical: 4 },
    li: { color: bodyColor, fontSize: bodySize, lineHeight: bodyLineHeight },
    table: { borderColor: theme.border, borderRadius: Radius.small, marginVertical: 7, overflow: 'hidden' },
    tableCell: { borderColor: theme.border, padding: 7 },
    image: { borderRadius: Radius.medium, maxWidth: '100%' },
  }), [bodyColor, bodyLineHeight, bodySize, compact, theme]);
  const markdownTheme = useMemo(() => ({
    colors: {
      border: theme.border,
      code: theme.inset,
      link: theme.accent,
      text: bodyColor,
    },
    spacing: { xs: 2, s: 4, m: 6, l: 12 },
  }), [bodyColor, theme]);
  const elements = useMarkdown(value, {
    colorScheme,
    styles,
    theme: markdownTheme,
  });

  return (
    <View style={sheet.content}>
      {elements.map((element, index) => (
        <Fragment key={`markdown-${index}`}>{element}</Fragment>
      ))}
    </View>
  );
});

const sheet = StyleSheet.create({
  content: { width: '100%' },
});
