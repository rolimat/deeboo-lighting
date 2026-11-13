import { useState } from "preact/hooks";
import { help } from "../shell/Help";
import { H } from "../help-texts";

export const USER_CODE_MESSAGE = "Enter the User Code from Smart Life (letters and digits only).";

export function StepUserCode({ onSubmit }: { onSubmit: (code: string) => Promise<void> }) {
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const submit = (e: Event) => {
    e.preventDefault();
    const trimmed = code.trim();
    if (!/^[A-Za-z0-9]{1,32}$/.test(trimmed)) {
      setError(USER_CODE_MESSAGE);
      return;
    }
    setError(null);
    setCode("");
    onSubmit(trimmed).catch((err: unknown) => setError(String(err)));
  };
  return (
    <form onSubmit={submit} class="space-y-3">
      <p>Open Smart Life → Me → Settings → Account and Security → User Code, then paste it here.</p>
      <input
        type="password"
        autocomplete="off"
        spellcheck={false}
        aria-label="User Code"
        {...help(H.userCode)}
        value={code}
        onInput={(e) => setCode(e.currentTarget.value)}
        class="w-full rounded-card border border-border bg-card px-3 py-2"
      />
      {error && (
        <p role="alert" class="text-red">
          {error}
        </p>
      )}
      <button type="submit" class="rounded-card bg-green px-4 py-2" {...help(H.userCodeContinue)}>
        Continue
      </button>
    </form>
  );
}
