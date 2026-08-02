/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // S5-fix: Use `rgb(var(--xxx-rgb) / <alpha-value>)` so Tailwind's
        // opacity modifier (e.g. `bg-primary/5`) works. The CSS variables
        // `--color-*-rgb` are defined as space-separated RGB triplets in
        // index.css (e.g. `--color-primary-rgb: 245 245 245;`).
        // The original hex variables (`--color-primary`) are retained for
        // direct `var()` usage in inline styles and custom CSS.
        primary: {
          DEFAULT: 'rgb(var(--color-primary-rgb) / <alpha-value>)',
          hover: 'rgb(var(--color-primary-hover-rgb) / <alpha-value>)',
          fg: 'rgb(var(--color-primary-fg-rgb) / <alpha-value>)',
        },
        accent: 'rgb(var(--color-accent-rgb) / <alpha-value>)',
        bg: {
          primary: 'rgb(var(--color-bg-primary-rgb) / <alpha-value>)',
          secondary: 'rgb(var(--color-bg-secondary-rgb) / <alpha-value>)',
          tertiary: 'rgb(var(--color-bg-tertiary-rgb) / <alpha-value>)',
          chrome: 'rgb(var(--color-bg-chrome-rgb) / <alpha-value>)',
        },
        text: {
          primary: 'rgb(var(--color-text-primary-rgb) / <alpha-value>)',
          secondary: 'rgb(var(--color-text-secondary-rgb) / <alpha-value>)',
        },
        border: {
          DEFAULT: 'rgb(var(--color-border-rgb) / <alpha-value>)',
          strong: 'rgb(var(--color-border-strong-rgb) / <alpha-value>)',
        },
        success: 'rgb(var(--color-success-rgb) / <alpha-value>)',
        warning: 'rgb(var(--color-warning-rgb) / <alpha-value>)',
        error: 'rgb(var(--color-error-rgb) / <alpha-value>)',
      },
      boxShadow: {
        elevated: 'var(--shadow-elevated)',
      },
      borderRadius: {
        ui: '0.75rem',
      },
      transitionTimingFunction: {
        out: 'cubic-bezier(0.22, 1, 0.36, 1)',
      },
    },
  },
  plugins: [],
}
