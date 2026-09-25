import type { ProviderModel } from '@padu/client';
import * as Haptics from 'expo-haptics';
import { useEffect, useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import Slider from '@react-native-community/slider';

import { NativeTint, Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';

export type ReasoningEffortOption = ProviderModel['reasoning_efforts'][number];

export interface ReasoningSliderProps {
  efforts: ReasoningEffortOption[];
  /** Currently selected effort id — controlled from outside. */
  selected: string | null;
  onSelect: (id: string) => void;
}

/**
 * Controlled snap-slider for reasoning effort using
 * @react-native-community/slider. The slider snaps to discrete steps
 * matching each effort option. The thumb colour and filled track use the
 * product accent colour; the empty track uses the muted overlay.
 *
 * Works identically on iOS and Android — no platform-specific code.
 */
export function ReasoningSlider({ efforts, selected, onSelect }: ReasoningSliderProps) {
  const theme = useTheme();
  const max = Math.max(0, efforts.length - 1);
  const selectedIndex = Math.max(0, efforts.findIndex((e) => e.id === selected));

  // Local integer index — keeps the slider thumb snapped while dragging.
  const [localIndex, setLocalIndex] = useState(selectedIndex);

  // Sync when the controlled `selected` prop changes externally
  // (e.g. picking a different model resets the default effort).
  useEffect(() => {
    setLocalIndex(selectedIndex);
  }, [selectedIndex]);

  if (!efforts.length) return null;

  const activeEffort = efforts[localIndex];

  return (
    <View style={styles.container}>
      {/* Header: label + live effort name */}
      <View style={styles.header}>
        <Text style={[styles.label, { color: theme.textTertiary }]}>
          REASONING EFFORT
        </Text>
        <Text style={[styles.value, { color: theme.accent }]}>
          {activeEffort?.label ?? ''}
        </Text>
      </View>

      {/* Slider */}
      <Slider
        accessibilityLabel="Reasoning effort"
        maximumTrackTintColor={theme.overlayStrong as string}
        maximumValue={max}
        minimumTrackTintColor={theme.accent as string}
        minimumValue={0}
        step={1}
        style={styles.slider}
        thumbTintColor={theme.accent as string}
        value={localIndex}
        onSlidingComplete={(value) => {
          const index = Math.round(value);
          const effort = efforts[index];
          if (!effort) return;
          setLocalIndex(index);
          void Haptics.selectionAsync();
          onSelect(effort.id);
        }}
        onValueChange={(value) => {
          setLocalIndex(Math.round(value));
        }}
      />

      {/* Step labels */}
      <View style={styles.stepLabels}>
        {efforts.map((effort, i) => (
          <Text
            key={effort.id}
            numberOfLines={1}
            style={[
              styles.stepLabel,
              {
                color: i === localIndex ? theme.accent : theme.textTertiary,
                fontWeight: i === localIndex ? '700' : '500',
              },
            ]}>
            {effort.label}
          </Text>
        ))}
      </View>

      {/* Active effort description */}
      {activeEffort?.description ? (
        <Text style={[styles.description, { color: theme.textTertiary }]}>
          {activeEffort.description}
        </Text>
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    borderTopWidth: StyleSheet.hairlineWidth,
    paddingBottom: Spacing.four,
    paddingHorizontal: Spacing.three,
    paddingTop: Spacing.three,
  },
  header: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
    marginBottom: Spacing.two,
  },
  label: {
    fontSize: 11,
    fontWeight: '600',
    letterSpacing: 0.5,
    textTransform: 'uppercase',
  },
  value: {
    fontSize: 13,
    fontWeight: '700',
  },
  slider: {
    marginHorizontal: -Spacing.one,
    height: 44,
    width: '100%',
  },
  stepLabels: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    marginTop: Spacing.one,
    paddingHorizontal: 2,
  },
  stepLabel: {
    fontSize: 11.5,
    textAlign: 'center',
  },
  description: {
    fontSize: 12,
    lineHeight: 17,
    marginTop: Spacing.two,
    textAlign: 'center',
  },
});
