import { useEffect } from "react";
import { Toaster } from "@/components/ui/sonner";
import { useSettings } from "@/hooks/use-settings";

/** Applies the saved theme to <html> and themes the toast host. */
export function ThemeSync() {
  const { data } = useSettings();
  const theme = data?.theme ?? "dark";

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = theme === "dark" || (theme === "system" && media.matches);
      document.documentElement.classList.toggle("dark", dark);
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);

  return <Toaster theme={theme} />;
}
