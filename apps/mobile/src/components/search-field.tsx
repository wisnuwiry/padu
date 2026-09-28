import { Pressable, StyleSheet, TextInput, View } from 'react-native';

import { PaduIcon } from '@/components/padu-icon';
import { NativeTint, Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';

/**
 * The task-list search field. It only exists while a search is open: the header
 * carries an icon button that reveals this, and the trailing control clears the
 * query first, then closes the field.
 */
export function SearchField({
  onChange,
  onClose,
  value,
}: {
  onChange: (value: string) => void;
  onClose: () => void;
  value: string;
}) {
  const theme = useTheme();
  return (
    <View style={[styles.field, { backgroundColor: theme.inset }]}>
      <PaduIcon
        name="search"
        size={16}
        tintColor={theme.textTertiary}
      />
      <TextInput
        accessibilityLabel="Search tasks"
        autoCapitalize="none"
        autoCorrect={false}
        autoFocus
        onChangeText={onChange}
        placeholder="Search tasks"
        placeholderTextColor={theme.textTertiary}
        returnKeyType="search"
        selectionColor={NativeTint}
        style={[styles.input, { color: theme.text }]}
        value={value}
      />
      <Pressable
        accessibilityLabel={value ? 'Clear search' : 'Close search'}
        accessibilityRole="button"
        hitSlop={8}
        onPress={() => (value ? onChange('') : onClose())}
        style={({ pressed }) => ({ opacity: pressed ? 0.5 : 1 })}>
        <PaduIcon
          name="x"
          size={16}
          tintColor={theme.textTertiary}
        />
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  field: {
    alignItems: 'center',
    borderRadius: Radius.medium,
    flexDirection: 'row',
    gap: Spacing.two,
    minHeight: 38,
    paddingHorizontal: Spacing.two,
  },
  input: { flex: 1, fontSize: 15.5, paddingVertical: Spacing.one },
});
