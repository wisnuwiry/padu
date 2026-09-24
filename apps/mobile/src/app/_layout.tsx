import { DarkTheme, DefaultTheme, Stack, ThemeProvider } from "expo-router";
import * as SplashScreen from "expo-splash-screen";
import * as SystemUI from "expo-system-ui";
import { StatusBar } from "expo-status-bar";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect } from "react";
import { StyleSheet } from "react-native";
import { GestureHandlerRootView } from "react-native-gesture-handler";

import { Colors } from "@/constants/theme";
import { useColorScheme } from "@/hooks/use-color-scheme";
import { AppearanceProvider } from "@/lib/appearance-context";
import { DaemonProvider, useDaemon } from "@/lib/daemon-context";
import { RuntimeProvider } from "@/lib/runtime-context";

void SplashScreen.preventAutoHideAsync();
SplashScreen.setOptions({ duration: 250, fade: true });

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnReconnect: true,
    },
  },
});

export default function RootLayout() {
  return (
    <GestureHandlerRootView style={styles.root}>
      <AppearanceProvider>
        <AppShell />
      </AppearanceProvider>
    </GestureHandlerRootView>
  );
}

/** Everything that renders theme colours sits below `AppearanceProvider`, so
 * the user's System / Light / Dark choice reaches the navigation theme, the
 * status bar, and every screen. */
function AppShell() {
  const colorScheme = useColorScheme();
  const colors = Colors[colorScheme];
  const navigationTheme =
    colorScheme === "dark"
      ? {
          ...DarkTheme,
          colors: {
            ...DarkTheme.colors,
            background: colors.background,
            card: colors.background,
          },
        }
      : {
          ...DefaultTheme,
          colors: {
            ...DefaultTheme.colors,
            background: colors.background,
            card: colors.background,
          },
        };

  // Screens are opaque, but the root view shows through while one animates in
  // — and on Android, behind the keyboard.
  useEffect(() => {
    void SystemUI.setBackgroundColorAsync(colors.background).catch(() => {});
  }, [colors.background]);

  return (
    <QueryClientProvider client={queryClient}>
      <DaemonProvider>
        <RuntimeProvider>
          <ThemeProvider value={navigationTheme}>
            <AppNavigator />
            <StatusBar style={colorScheme === "dark" ? "light" : "dark"} />
          </ThemeProvider>
        </RuntimeProvider>
      </DaemonProvider>
    </QueryClientProvider>
  );
}

function AppNavigator() {
  const { phase } = useDaemon();
  const theme = Colors[useColorScheme()];

  useEffect(() => {
    if (phase !== "booting") void SplashScreen.hideAsync();
  }, [phase]);

  return (
    <Stack
      screenOptions={{
        contentStyle: { backgroundColor: theme.background },
        headerBackButtonDisplayMode: "minimal",
        headerShadowVisible: false,
        headerStyle: { backgroundColor: theme.background },
        headerTintColor: theme.text,
      }}
    >
      <Stack.Screen name="index" options={{ headerShown: false, title: "Padu" }} />
      <Stack.Screen
        name="daemons"
        options={{ headerShown: false, title: "Daemons" }}
      />
      <Stack.Screen name="new-task" options={{ headerShown: false }} />
      <Stack.Screen
        name="daemon-editor"
        options={{
          presentation: "pageSheet",
          title: "Add Daemon",
        }}
      />
      <Stack.Screen name="session/[id]" options={{ headerShown: false }} />
    </Stack>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1 },
});
