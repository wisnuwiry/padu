import { SymbolView, type SymbolViewProps } from 'expo-symbols';
import { StyleSheet, View, type ViewProps } from 'react-native';

interface AppSymbolProps extends Omit<SymbolViewProps, 'size' | 'style'> {
  size?: number;
  style?: ViewProps['style'];
}

export function AppSymbol({ size = 18, style, ...props }: AppSymbolProps) {
  return (
    <View style={[styles.frame, { width: size, height: size }, style]}>
      <SymbolView resizeMode="scaleAspectFit" size={size} {...props} />
    </View>
  );
}

const styles = StyleSheet.create({
  frame: {
    alignItems: 'center',
    justifyContent: 'center',
  },
});
