import { useEffect, useState } from "react";
import { Modal } from "@storyteller/ui-modal";
import { Button } from "@storyteller/ui-button";
import { Input } from "@storyteller/ui-input";
import { Select } from "@storyteller/ui-select";
import { ChevronLeft, Globe, Key, Plus, UserPlus } from "lucide-react";
import {
  API_KEY_SERVICES,
  ServiceMeta,
  WEBSITE_LOGIN_SERVICES,
  addApiCredential,
  getServiceLogoPath,
  openWebLogin,
} from "./credential-helpers";

/** Which screen the modal shows. */
type AddCredentialView = "choice" | "api_key" | "website_login";

const ARTCRAFT_TAGLINE = "The Open Platform for Generative Creators.";

interface AddCredentialModalProps {
  isOpen: boolean;
  onClose: () => void;
  onAdded: () => void;
  /**
   * Called for services with our own username/password form (ArtCraft):
   * the caller closes this modal and opens the login form instead.
   */
  onChooseArtcraftLogin: (service: ServiceMeta) => void;
}

/**
 * The "Add credential" modal. Opens on a three-way choice — a big ArtCraft
 * account section on top, with API key and website login side by side below —
 * then drills into the API-key form or the website-login chooser, each with a
 * back link to the choice. Escape / clicking out closes the modal from any
 * view.
 */
export const AddCredentialModal = ({
  isOpen,
  onClose,
  onAdded,
  onChooseArtcraftLogin,
}: AddCredentialModalProps) => {
  const [view, setView] = useState<AddCredentialView>("choice");

  // Start over each time the modal opens.
  useEffect(() => {
    if (isOpen) setView("choice");
  }, [isOpen]);

  const artcraftMeta = WEBSITE_LOGIN_SERVICES.find(
    (meta) => meta.value === "artcraft",
  );

  const titles: Record<AddCredentialView, string> = {
    choice: "Add credential",
    api_key: "Add API key",
    website_login: "Add website login",
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={titles[view]}
      titleIcon={view === "website_login" ? Globe : view === "api_key" ? Key : Plus}
      className="max-w-xl"
      showClose={true}
    >
      {view === "choice" && (
        <ChoiceView
          onChooseArtcraft={() => {
            if (artcraftMeta) onChooseArtcraftLogin(artcraftMeta);
          }}
          onChooseApiKey={() => setView("api_key")}
          onChooseWebsiteLogin={() => setView("website_login")}
        />
      )}
      {view === "api_key" && (
        <ApiKeyView
          onBack={() => setView("choice")}
          onAdded={onAdded}
          onClose={onClose}
        />
      )}
      {view === "website_login" && (
        <WebsiteLoginView
          onBack={() => setView("choice")}
          onClose={onClose}
          onChooseArtcraftLogin={onChooseArtcraftLogin}
        />
      )}
    </Modal>
  );
};

// ── Views ──

/** "< Back" link returning to the three-way choice. */
const BackLink = ({ onClick }: { onClick: () => void }) => (
  <button
    className="-ml-1 flex w-fit items-center gap-0.5 text-sm text-base-fg/50 transition-colors hover:text-base-fg"
    onClick={onClick}
  >
    <ChevronLeft size="1.1em" />
    Back
  </button>
);

const ChoiceView = ({
  onChooseArtcraft,
  onChooseApiKey,
  onChooseWebsiteLogin,
}: {
  onChooseArtcraft: () => void;
  onChooseApiKey: () => void;
  onChooseWebsiteLogin: () => void;
}) => (
  <div className="flex flex-col gap-3 p-1 text-base-fg">
    {/* The big ArtCraft option. */}
    <div
      className="group flex cursor-pointer flex-col items-center gap-3 rounded-xl border border-ui-panel-border bg-ui-controls/40 px-6 py-8 text-center transition-colors hover:bg-ui-controls"
      role="button"
      tabIndex={0}
      onClick={onChooseArtcraft}
      onKeyDown={(e) => {
        if (e.key === "Enter") onChooseArtcraft();
      }}
    >
      <img
        src={getServiceLogoPath("artcraft")}
        alt="ArtCraft logo"
        className="h-14 object-contain icon-auto-contrast"
      />
      <div className="flex flex-col gap-1">
        <span className="text-lg font-semibold">Add ArtCraft Account</span>
        <span className="text-sm text-base-fg/60">{ARTCRAFT_TAGLINE}</span>
      </div>
      <Button
        variant="primary"
        className="h-9 px-5"
        icon={UserPlus}
        onClick={(e: React.MouseEvent) => {
          e.stopPropagation();
          onChooseArtcraft();
        }}
      >
        Sign in
      </Button>
    </div>

    {/* The two smaller options, side by side. */}
    <div className="grid grid-cols-2 gap-3">
      <SmallChoiceCard
        icon={Key}
        title="Add API Key"
        subtitle="FAL, Replicate, OpenRouter, etc."
        buttonLabel="Add key"
        onChoose={onChooseApiKey}
      />
      <SmallChoiceCard
        icon={Globe}
        title="Add Website Login"
        subtitle="Midjourney, Grok, Runway, Higgsfield, etc."
        buttonLabel="Log in"
        onChoose={onChooseWebsiteLogin}
      />
    </div>
  </div>
);

const SmallChoiceCard = ({
  icon: Icon,
  title,
  subtitle,
  buttonLabel,
  onChoose,
}: {
  icon: typeof Key;
  title: string;
  subtitle: string;
  buttonLabel: string;
  onChoose: () => void;
}) => (
  <div
    className="group flex cursor-pointer flex-col items-center gap-2 rounded-xl border border-ui-panel-border bg-ui-controls/40 px-4 py-5 text-center transition-colors hover:bg-ui-controls"
    role="button"
    tabIndex={0}
    onClick={onChoose}
    onKeyDown={(e) => {
      if (e.key === "Enter") onChoose();
    }}
  >
    <Icon size="1.4em" className="text-base-fg/60" />
    <div className="flex flex-col gap-0.5">
      <span className="text-sm font-medium">{title}</span>
      <span className="text-xs text-base-fg/50">{subtitle}</span>
    </div>
    <Button
      variant="secondary"
      className="mt-1 h-8 px-4"
      onClick={(e: React.MouseEvent) => {
        e.stopPropagation();
        onChoose();
      }}
    >
      {buttonLabel}
    </Button>
  </div>
);

/**
 * The pre-existing API key form: pick a key type (ArtCraft API key included)
 * and paste the key, with an optional name.
 */
const ApiKeyView = ({
  onBack,
  onAdded,
  onClose,
}: {
  onBack: () => void;
  onAdded: () => void;
  onClose: () => void;
}) => {
  const [service, setService] = useState<string>(API_KEY_SERVICES[0].value);
  const [apiKey, setApiKey] = useState("");
  const [name, setName] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");

  const handleAdd = async () => {
    if (isSaving || !apiKey.trim()) return;
    setIsSaving(true);
    setErrorMessage("");
    try {
      await addApiCredential({
        service,
        apiKey: apiKey.trim(),
        name: name.trim() || undefined,
      });
      onAdded();
      onClose();
    } catch (e) {
      console.error("Failed to add credential", e);
      setErrorMessage(String(e));
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="flex flex-col gap-4 p-1 text-base-fg">
      <BackLink onClick={onBack} />

      <div className="flex flex-col gap-1.5">
        <label className="text-sm text-base-fg/70">API key type</label>
        <div className="flex items-center gap-2.5">
          <img
            src={getServiceLogoPath(service)}
            alt=""
            className="h-6 w-6 shrink-0 object-contain icon-auto-contrast"
          />
          <Select
            className="grow"
            value={service}
            onChange={(value) => setService(String(value))}
            options={API_KEY_SERVICES.map((meta) => ({
              label: meta.label,
              value: meta.value,
            }))}
          />
        </div>
      </div>

      <div className="flex flex-col gap-1.5">
        <label className="text-sm text-base-fg/70">API key</label>
        <Input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey((e.target as HTMLInputElement).value)}
          placeholder="Paste your API key"
          autoFocus
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

      <Button
        variant="primary"
        className="h-9"
        onClick={handleAdd}
        disabled={isSaving || !apiKey.trim()}
      >
        {isSaving ? "Adding..." : "Add"}
      </Button>
    </div>
  );
};

/**
 * Website login chooser: one logo button per site. Most sites open a backend
 * login webview (the modal closes so the user can complete login there);
 * ArtCraft services switch to the username/password form instead.
 */
const WebsiteLoginView = ({
  onBack,
  onClose,
  onChooseArtcraftLogin,
}: {
  onBack: () => void;
  onClose: () => void;
  onChooseArtcraftLogin: (service: ServiceMeta) => void;
}) => {
  const handleLogin = async (meta: ServiceMeta) => {
    if (meta.passwordLogin) {
      onChooseArtcraftLogin(meta);
      return;
    }
    if (!meta.loginWebsite) return;
    try {
      await openWebLogin(meta.loginWebsite);
      onClose();
    } catch (error) {
      console.error(`Failed to open login window for ${meta.loginWebsite}:`, error);
    }
  };

  return (
    <div className="flex flex-col gap-3 p-1 text-base-fg">
      <BackLink onClick={onBack} />
      <p className="text-sm text-base-fg/60">
        Pick a website to log into. Your session cookies will be stored as a
        credential.
      </p>
      <div className="grid grid-cols-2 gap-2.5">
        {WEBSITE_LOGIN_SERVICES.map((meta) => (
          <button
            key={meta.value}
            className="flex h-16 flex-col items-center justify-center gap-1.5 rounded-lg border border-ui-panel-border bg-ui-controls/40 transition-colors hover:bg-ui-controls"
            onClick={() => handleLogin(meta)}
          >
            <img
              src={getServiceLogoPath(meta.value)}
              alt={`${meta.label} logo`}
              className="h-6 max-w-[120px] object-contain icon-auto-contrast"
            />
            <span className="text-xs text-base-fg/70">{meta.label}</span>
          </button>
        ))}
      </div>
    </div>
  );
};
