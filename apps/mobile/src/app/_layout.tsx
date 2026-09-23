import { DarkTheme, DefaultTheme, Stack, ThemeProvider } from "expo-router";
import * as SplashScreen from "expo-splash-screen";
import { StatusBar } from "expo-status-bar";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect } from "react";
import { StyleSheet, useColorScheme } from "react-native";
import { GestureHandlerRootView } from "react-native-gesture-handler";

import { Colors } from "@/constants/theme";
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
  const colorScheme = useColorScheme();
  const colors = Colors[colorScheme === "dark" ? "dark" : "light"];
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
  return (
    <GestureHandlerRootView style={styles.root}>
      <QueryClientProvider client={queryClient}>
        <DaemonProvider>
          <RuntimeProvider>
            <ThemeProvider value={navigationTheme}>
              <AppNavigator />
              <StatusBar style="auto" />
            </ThemeProvider>
          </RuntimeProvider>
        </DaemonProvider>
      </QueryClientProvider>
    </GestureHandlerRootView>
  );
}

function AppNavigator() {
  const { phase } = useDaemon();
  const colorScheme = useColorScheme();
  const theme = Colors[colorScheme === "dark" ? "dark" : "light"];

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
