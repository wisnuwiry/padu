import { Image, StyleSheet, View } from 'react-native';
import { Defs, LinearGradient, Rect, Stop, Svg } from 'react-native-svg';

import { useConversationBackground } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';

/**
 * Read-only port of web's `ConversationBackgroundLayer`: the configured
 * image pinned to the top of the transcript at `heightPercent` height and
 * `opacity`, fading into the theme background (SVG gradient — no extra
 * native dependency). Decorative and touch-transparent; renders nothing
 * while no image is configured, mirroring web's early return.
 */

export function ConversationBackgroundLayer() {
  const theme = useTheme();
  const background = useConversationBackground();
  if (!background.imageUrl || background.loading) return null;
  return (
    <View pointerEvents="none" style={styles.layer}>
      <Image
        accessible={false}
        resizeMode="cover"
        source={{ uri: background.imageUrl }}
        style={[styles.image, { height: `${background.heightPercent}%`, opacity: background.opacity }]}
      />
      <Svg style={[styles.fade, { height: `${background.heightPercent}%` }]}>
        <Defs>
          <LinearGradient id="conversation-background-fade" x1="0" x2="0" y1="0" y2="1">
            <Stop offset="0" stopColor={theme.background} stopOpacity="0" />
            <Stop offset="1" stopColor={theme.background} stopOpacity="0.95" />
          </LinearGradient>
        </Defs>
        <Rect fill="url(#conversation-background-fade)" height="100%" width="100%" x="0" y="0" />
      </Svg>
    </View>
  );
}

const styles = StyleSheet.create({
  layer: {
    bottom: 0,
    left: 0,
    position: 'absolute',
    right: 0,
    top: 0,
  },
  image: { width: '100%' },
  fade: { position: 'absolute', top: 0, width: '100%' },
});
