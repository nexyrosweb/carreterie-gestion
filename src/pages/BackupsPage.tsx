import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import { apiBackupCreate, apiBackupList, apiBackupRestore, ApiError } from "@/lib/api";
import type { BackupInfo } from "@/types";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import { Input } from "@/components/ui/Input";
import { Modal } from "@/components/ui/Modal";
import { Spinner } from "@/components/ui/Spinner";
import { TBody, THead, Table, Td, Th, Tr } from "@/components/ui/Table";
import { format } from "date-fns";
import { fr } from "date-fns/locale";

function pathFileName(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] ?? path;
}

export function BackupsPage() {
  const { sessionToken } = useAuth();
  const { showToast } = useToast();
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [createOpen, setCreateOpen] = useState(false);
  const [restoreOpen, setRestoreOpen] = useState(false);
  const [restorePath, setRestorePath] = useState("");
  const [password, setPassword] = useState("");
  const [useEncryption, setUseEncryption] = useState(false);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    if (!sessionToken) return;
    setLoading(true);
    try {
      setBackups(await apiBackupList(sessionToken));
    } catch {
      setBackups([]);
    } finally {
      setLoading(false);
    }
  }, [sessionToken]);

  useEffect(() => {
    void load();
  }, [load]);

  async function createBackup() {
    if (!sessionToken) return;
    if (useEncryption && password.length < 8) {
      showToast("Mot de passe de chiffrement : 8 caractères minimum.", "warning");
      return;
    }
    setBusy(true);
    try {
      const info = await apiBackupCreate(
        sessionToken,
        useEncryption && password ? password : undefined,
      );
      showToast(`Sauvegarde créée : ${pathFileName(info.path)}`, "success");
      setCreateOpen(false);
      setPassword("");
      setUseEncryption(false);
      await load();
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Échec de la sauvegarde.", "error");
    } finally {
      setBusy(false);
    }
  }

  async function pickRestoreFile() {
    const selected = await open({
      multiple: false,
      title: "Choisir une sauvegarde à restaurer",
      filters: [{ name: "Sauvegarde", extensions: ["zip", "carreterie"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    setRestorePath(selected);
    setRestoreOpen(true);
  }

  async function restoreBackup() {
    if (!sessionToken || !restorePath) return;
    setBusy(true);
    try {
      await apiBackupRestore(
        sessionToken,
        restorePath,
        password ? password : undefined,
      );
      showToast("Restauration terminée. Rechargez l'application si nécessaire.", "success");
      setRestoreOpen(false);
      setRestorePath("");
      setPassword("");
      await load();
    } catch (err) {
      showToast(err instanceof ApiError ? err.message : "Restauration impossible.", "error");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-4">
      <p className="text-sm text-[var(--color-text-muted)]">
        Les sauvegardes peuvent être chiffrées avec un mot de passe optionnel. Conservez-le en lieu
        sûr, distinct du mot de passe de connexion.
      </p>

      <Card
        title="Sauvegardes"
        action={
          <div className="flex gap-2">
            <Button size="sm" variant="secondary" onClick={() => void pickRestoreFile()}>
              Restaurer depuis un fichier…
            </Button>
            <Button size="sm" onClick={() => setCreateOpen(true)}>
              Créer une sauvegarde
            </Button>
          </div>
        }
      >
        {loading ? (
          <Spinner className="mx-auto py-10 text-[var(--color-or)]" />
        ) : backups.length === 0 ? (
          <EmptyState
            title="Aucune sauvegarde listée"
            description="Créez une sauvegarde ou restaurez depuis un fichier (backup_list peut être absent côté backend)."
            action={<Button onClick={() => setCreateOpen(true)}>Créer</Button>}
          />
        ) : (
          <Table>
            <THead>
              <tr>
                <Th>Fichier</Th>
                <Th>Date</Th>
                <Th>Chiffrée</Th>
                <Th className="text-right">Actions</Th>
              </tr>
            </THead>
            <TBody>
              {backups.map((b) => (
                <Tr key={b.path}>
                  <Td className="text-xs">{pathFileName(b.path)}</Td>
                  <Td>{format(new Date(b.created_at), "dd/MM/yyyy HH:mm", { locale: fr })}</Td>
                  <Td>{b.encrypted ? "Oui" : "Non"}</Td>
                  <Td className="text-right">
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => {
                        setRestorePath(b.path);
                        setPassword("");
                        setRestoreOpen(true);
                      }}
                    >
                      Restaurer
                    </Button>
                  </Td>
                </Tr>
              ))}
            </TBody>
          </Table>
        )}
      </Card>

      <Modal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        title="Nouvelle sauvegarde"
        footer={
          <>
            <Button variant="secondary" onClick={() => setCreateOpen(false)}>
              Annuler
            </Button>
            <Button onClick={() => void createBackup()} loading={busy}>
              Créer
            </Button>
          </>
        }
      >
        <label className="mb-4 flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={useEncryption}
            onChange={(e) => setUseEncryption(e.target.checked)}
          />
          Chiffrer avec un mot de passe
        </label>
        {useEncryption ? (
          <Input
            type="password"
            label="Mot de passe de chiffrement"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            hint="Minimum 8 caractères."
          />
        ) : (
          <p className="text-xs text-[var(--color-text-muted)]">
            Sans mot de passe, l'archive ne sera pas chiffrée.
          </p>
        )}
      </Modal>

      <Modal
        open={restoreOpen}
        onClose={() => setRestoreOpen(false)}
        title="Restaurer une sauvegarde"
        footer={
          <>
            <Button variant="secondary" onClick={() => setRestoreOpen(false)}>
              Annuler
            </Button>
            <Button variant="danger" onClick={() => void restoreBackup()} loading={busy}>
              Restaurer
            </Button>
          </>
        }
      >
        <p className="mb-4 text-sm text-[var(--color-text-muted)]">
          Fichier : <span className="text-[var(--color-text)]">{pathFileName(restorePath)}</span>
        </p>
        <Input
          type="password"
          label="Mot de passe (si la sauvegarde est chiffrée)"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
      </Modal>
    </div>
  );
}