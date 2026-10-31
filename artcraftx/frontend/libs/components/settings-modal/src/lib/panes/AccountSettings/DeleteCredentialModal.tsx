import { useEffect, useState } from "react";
import { Modal } from "@storyteller/ui-modal";
import { Button } from "@storyteller/ui-button";
import { Trash2 } from "lucide-react";
import {
  CredentialPayload,
  deleteCredential,
  getServiceMeta,
} from "./credential-helpers";

interface DeleteCredentialModalProps {
  credential: CredentialPayload | null;
  onClose: () => void;
  onDeleted: () => void;
}

/** Confirmation dialog before deleting a credential file. */
export const DeleteCredentialModal = ({
  credential,
  onClose,
  onDeleted,
}: DeleteCredentialModalProps) => {
  const [isDeleting, setIsDeleting] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");

  useEffect(() => {
    if (credential) {
      setIsDeleting(false);
      setErrorMessage("");
    }
  }, [credential]);

  if (!credential) return null;

  const handleDelete = async () => {
    if (isDeleting) return;
    setIsDeleting(true);
    setErrorMessage("");
    try {
      // Deletes by file name (the credential id) so we're sure we're
      // removing exactly this one.
      await deleteCredential(credential.id);
      onDeleted();
      onClose();
    } catch (e) {
      console.error("Failed to delete credential", e);
      setErrorMessage(String(e));
      setIsDeleting(false);
    }
  };

  const meta = getServiceMeta(credential.service);
  const label = credential.name || meta.label;

  return (
    <Modal
      isOpen={true}
      onClose={onClose}
      title="Delete credential"
      titleIcon={Trash2}
      width="w-[420px]"
      showClose={true}
    >
      <div className="flex flex-col gap-4 p-1 text-base-fg">
        <p className="text-sm text-base-fg/80">
          Delete the credential <span className="font-medium">{label}</span>?
        </p>
        <p className="text-sm text-base-fg/50">
          This removes <span className="font-mono">{credential.file_name}</span> from
          your credentials folder. This cannot be undone.
        </p>

        {errorMessage && (
          <div className="text-sm text-red-400">{errorMessage}</div>
        )}

        <div className="flex justify-end gap-2">
          <Button variant="secondary" className="h-9" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="destructive"
            className="h-9"
            onClick={handleDelete}
            disabled={isDeleting}
          >
            {isDeleting ? "Deleting..." : "Delete"}
          </Button>
        </div>
      </div>
    </Modal>
  );
};
