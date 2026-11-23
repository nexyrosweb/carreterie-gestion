import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@tauri-apps/api/core";
import { apiAppDisclaimer, ApiError } from "@/lib/api";
import { TheatreLogo } from "@/components/brand/TheatreLogo";
import { Card } from "@/components/ui/Card";
import { Spinner } from "@/components/ui/Spinner";

export function AboutPage() {
  const [disclaimer, setDisclaimer] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [version, setVersion] = useState("…");

  useEffect(() => {
    void (async () => {
      try {
        setDisclaimer(await apiAppDisclaimer());
      } catch (err) {
        setError(err instanceof ApiError ? err.message : "Informations indisponibles.");
      }
    })();
  }, []);

  useEffect(() => {
    if (!isTauri()) {
      setVersion("1.0.0");
      return;
    }
    void getVersion().then(setVersion).catch(() => setVersion("1.0.0"));
  }, []);

  if (disclaimer === null && !error) {
    return <Spinner className="mx-auto py-20 text-[var(--color-or)]" />;
  }

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <header className="flex flex-col items-start gap-4 sm:flex-row sm:items-center">
        <TheatreLogo className="max-w-[220px]" />
        <div>
          <h2 className="text-xl font-bold text-[var(--color-text)]">Carreterie Gestion</h2>
          <p className="text-sm text-[var(--color-text-muted)]">
            Gestion locale pour le Théâtre La Carreterie · mises à jour automatiques
          </p>
        </div>
      </header>

      <Card title="Éditeur">
        <p className="text-sm">
          Application développée par{" "}
          <span className="font-semibold text-[var(--color-or)]">Nexyros</span>.
        </p>
        <p className="mt-2 text-xs text-[var(--color-text-muted)]">
          Version {version} · Développé par Nexyros
        </p>
      </Card>

      <Card title="Mentions importantes">
        <ul className="list-disc space-y-2 pl-5 text-sm text-[var(--color-text-muted)]">
          <li>{disclaimer}</li>
          <li>
            Les données de production et de démonstration sont stockées séparément sous AppData
            Windows.
          </li>
        </ul>
      </Card>

      {error ? <p className="text-sm text-[var(--color-danger)]">{error}</p> : null}
    </div>
  );
}
