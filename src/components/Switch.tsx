interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

export default function Switch({ checked, onChange, disabled = false }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => !disabled && onChange(!checked)}
      className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors duration-150 ease-out ${
        disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'
      } ${checked ? 'bg-primary' : 'bg-bg-tertiary border border-border'}`}
    >
      <span
        className={`inline-block h-4 w-4 transform rounded-full transition-transform duration-150 ease-out shadow-sm ${
          checked ? 'translate-x-6 bg-primary-fg' : 'translate-x-1 bg-text-secondary'
        }`}
      />
    </button>
  );
}
