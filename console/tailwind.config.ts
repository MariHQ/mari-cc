import type { Config } from "tailwindcss";

export default {
  darkMode: ["class"],
  content: ["./src/**/*.{ts,tsx}", "./index.html", "./console.html"],
  prefix: "",
  theme: {
    container: {
      center: true,
      padding: "2rem",
      screens: {
        "2xl": "1400px",
      },
    },
    extend: {
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        surface: {
          DEFAULT: "hsl(var(--surface))",
          2: "hsl(var(--surface-2))",
          3: "hsl(var(--surface-3))",
        },
        emerald2: "hsl(var(--emerald))",
        purple2: "hsl(var(--purple))",
        amber2: "hsl(var(--amber))",
        rose2: "hsl(var(--rose))",
        pine: "hsl(var(--pine))",
        "basque-red": "hsl(var(--basque-red))",
        terracotta: "hsl(var(--terracotta))",
        seafoam: "hsl(var(--seafoam))",
        "gold-leaf": "hsl(var(--gold-leaf))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
          glow: "hsl(var(--primary-glow))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
        sidebar: {
          DEFAULT: "hsl(var(--sidebar-background))",
          foreground: "hsl(var(--sidebar-foreground))",
          primary: "hsl(var(--sidebar-primary))",
          "primary-foreground": "hsl(var(--sidebar-primary-foreground))",
          accent: "hsl(var(--sidebar-accent))",
          "accent-foreground": "hsl(var(--sidebar-accent-foreground))",
          border: "hsl(var(--sidebar-border))",
          ring: "hsl(var(--sidebar-ring))",
        },
        // --- Brutalist Blueprint ("Modern Basque") tokens · /redesign ---
        ink: "rgb(var(--c-ink) / <alpha-value>)",
        "ink-elev": "rgb(var(--c-ink-elev) / <alpha-value>)",
        paper: "rgb(var(--c-paper) / <alpha-value>)",
        flysch: "rgb(var(--c-flysch) / <alpha-value>)",
        biscay: { DEFAULT: "rgb(var(--c-biscay) / <alpha-value>)", 2: "rgb(var(--c-biscay-2) / <alpha-value>)" },
        espelette: "rgb(var(--c-espelette) / <alpha-value>)",
        // Status accents for the console — mossy green (ok) and burnt clay
        // (attention). Coastal palette, not Tailwind's default rainbow.
        moss: "#2C6E49",
        clay: "#A05E1C",
      },
      fontFamily: {
        display: ["Inter", "Helvetica Neue", "Arial", "sans-serif"],
        term: ['"JetBrains Mono"', "ui-monospace", "SFMono-Regular", "Menlo", "monospace"],
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
        xl: "var(--radius-xl)",
        pill: "999px",
      },
      keyframes: {
        "accordion-down": {
          from: {
            height: "0",
          },
          to: {
            height: "var(--radix-accordion-content-height)",
          },
        },
        "accordion-up": {
          from: {
            height: "var(--radix-accordion-content-height)",
          },
          to: {
            height: "0",
          },
        },
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
} satisfies Config;
