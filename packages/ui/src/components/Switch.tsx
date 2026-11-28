type SwitchProps = {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
};

/** Accessible toggle. `label` is announced to screen readers. */
export function Switch({ checked, onChange, label, disabled }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      className="vc-switch"
      data-checked={checked || undefined}
      onClick={() => onChange(!checked)}
    >
      <span className="vc-switch__thumb" aria-hidden="true" />
    </button>
  );
}
