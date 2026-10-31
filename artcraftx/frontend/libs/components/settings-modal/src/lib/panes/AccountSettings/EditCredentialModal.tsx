import { useEffect, useState } from "react";
import { Modal } from "@storyteller/ui-modal";
import { Button } from "@storyteller/ui-button";
import { Input } from "@storyteller/ui-input";
import { Key } from "lucide-react";
import {
  CredentialPayload,
  editApiCredential,
  getServiceLogoPath,
  getServiceMeta,
} from "./credential-helpers";

interface EditCredentialModalProps {
  credential: CredentialPayload | null;
  onClose: () => void;
  onSaved: () => void;
}

/** Edit an API-key credential: change the key and/or its optional name. */
export const EditCredentialModal = ({
  credential,
  onClose,
  onSaved,
}: EditCredentialModalProps) => {
  const [apiKey, setApiKey] = useState("");
  const [name, setName] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  // The key is masked at rest and revealed only while the input is focused.
  const [isKeyFocused, setIsKeyFocused] = useState(false);

  useEffect(() => {
    if (credential) {
      setApiKey(credential.api_key ?? "");
      setName(credential.name ?? "");
      setIsSaving(false);
      setErrorMessage("");
      setIsKeyFocused(false);
    }
  }, [credential]);

  if (!credential) return null;

  const handleSave = async () => {
    if (isSaving || !apiKey.trim()) return;
    setIsSaving(true);
    setErrorMessage("");
    try {
      await editApiCredential({
        credentialId: credential.id,
        apiKey: apiKey.trim(),
        // Empty string clears the name on the Rust side.
        name: name.trim(),
      });
      onSaved();
      onClose();
    } catch (e) {
      console.error("Failed to edit credential", e);
      setErrorMessage(String(e));
    } finally {
      setIsSaving(false);
    }
  };

  const meta = getServiceMeta(credential.service);

  return (
    <Modal
      isOpen={true}
      onClose={onClose}
      title={`Edit ${meta.label} API key`}
      titleIcon={Key}
      width="w-[440px]"
      showClose={true}
    >
      <div className="flex flex-col gap-4 p-1 text-base-fg">
        <div className="flex items-center gap-2.5 text-sm text-base-fg/60">
          <img
            src={getServiceLogoPath(credential.service)}
            alt=""
            className="h-5 w-5 shrink-0 object-contain icon-auto-contrast"
          />
          <span className="font-mono">{credential.file_name}</span>
        </div>

        <div className="flex flex-col gap-1.5">
          <label className="text-sm text-base-fg/70">API key</label>
          <Input
            type={isKeyFocused ? "text" : "password"}
            value={apiKey}
            onChange={(e) => setApiKey((e.target as HTMLInputElement).value)}
            onFocus={() => setIsKeyFocused(true)}
            onBlur={() => setIsKeyFocused(false)}
            placeholder="API key"
          />
        </div>

        <div className="flex flex-col gap-1.5">
          <label className="text-sm text-base-fg/70">Name (optional)</label>
          <Input
            value={name}
            onChange={(e) => setName((e.target as HTMLInputElement).value)}
            placeholder="e.g. work account"
          />
        </div>

        {errorMessage && (
          <div className="text-sm text-red-400">{errorMessage}</div>
        )}

        <div className="flex justify-end gap-2">
          <Button variant="secondary" className="h-9" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="primary"
            className="h-9"
            onClick={handleSave}
            disabled={isSaving || !apiKey.trim()}
          >
            {isSaving ? "Saving..." : "Save"}
          </Button>
        </div>
      </div>
    </Modal>
  );
};
