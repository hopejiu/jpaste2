/**
 * Toggle switch component matching old project style.
 * A reusable on/off toggle button.
 */
export function ToggleSwitch({ checked, onChange }: { checked: boolean; onChange: () => void }) {
  return (
    <button
      class={`toggle-switch ${checked ? 'on' : ''}`}
      onClick={onChange}
      role="switch"
      aria-checked={checked}
    >
      <div class="toggle-thumb" />
    </button>
  );
}
