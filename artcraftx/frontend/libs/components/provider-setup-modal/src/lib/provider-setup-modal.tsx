import { Modal } from "@storyteller/ui-modal";
import { invoke } from "@tauri-apps/api/core";
import { ArrowRight, WandSparkles } from "lucide-react";
import { Button } from "@storyteller/ui-button";
import { useState } from "react";
import { useShowProviderLoginModalEvent } from "@storyteller/tauri-events";
import { GenerationProvider } from "@storyteller/common";

interface ProviderSetupModalProps {
}

export function ProviderSetupModal({
}: ProviderSetupModalProps) {
  const [showModal, setShowModal] = useState(false);
  const [provider, setProvider] = useState<GenerationProvider>(GenerationProvider.Artcraft);

  const serviceProviderName = getServiceProviderName(provider);

  const modalTitle = `Set up ${serviceProviderName}`;
  const modalSubTitle = `Add your ${serviceProviderName} account to ArtCraft!`;

  let modalDescription;
  switch (provider) {
    case GenerationProvider.Grok:
    case GenerationProvider.Higgsfield:
      modalDescription = `You can add your ${serviceProviderName} account to ArtCraft by simply logging in. Use can then use it directly within Artcraft. You can add all of your AI accounts to Artcraft to use them all in one place and build the ultimate AI art tool.`;
      break;
    default:
      modalDescription = `You can add your ${serviceProviderName} account to ArtCraft by simply logging in. Use your credits and account directly within Artcraft. You can add all of your AI accounts to Artcraft to use them all in one place and build the ultimate AI art tool.`;
      break;
  }

  const modalButtonText = `Set up ${serviceProviderName}`;

  const buttonOnClick = async () => {
    switch (provider) {
      case GenerationProvider.Grok:
        await invoke("grok_open_login_command");
        break;
      case GenerationProvider.Higgsfield:
        // The generic website-login window (see `LoginWebsite::Higgsfield`).
        await invoke("open_web_login_command", { website: "higgsfield" });
        break;
      case GenerationProvider.Midjourney:
        await invoke("midjourney_open_login_command");
        break;
      case GenerationProvider.Fal:
        break; // TODO: None yet.
      default:
        break;
    }
    setShowModal(false);
  };

  return (
    <Modal
      //title={modalTitle}
      isOpen={showModal}
      onClose={() => {
        setShowModal(false);
      }}
      className="max-w-2xl max-h-[500px] p-6"
      showClose={true}
    >
      <div className="flex flex-col items-center justify-center gap-6">
        <div className="flex flex-col items-center gap-3">

          <br />

          <h1 className="text-3xl font-bold">
            <WandSparkles
              size="1em"
              className="mr-3 text-[24px]"
            />
            {modalTitle}
          </h1>
          <div className="text-center">
            <p className="text-lg font-medium text-white/80">{modalSubTitle}</p>

            <br />

            <p className="text-white/60">{modalDescription}</p>

            <br />

          </div>
        </div>

        {/*<div className="aspect-video w-full overflow-hidden rounded-md">
          Test
        </div>*/}
        <Button
          className="font-semibold"
          icon={ArrowRight}
          iconFlip={true}
          onClick={() => {
            buttonOnClick();
          }}
        >
          {modalButtonText}
        </Button>
      </div>
    </Modal>
  );
}

function getServiceProviderName(provider: GenerationProvider) : string {
  switch (provider) {
    case GenerationProvider.Grok:
      return "Grok";
    case GenerationProvider.Higgsfield:
      return "Higgsfield";
    case GenerationProvider.Fal:
      return "Fal";
    case GenerationProvider.Midjourney:
      return "Midjourney";
    case GenerationProvider.Artcraft:
    default:
      return "Artcraft";
  }
}

export default ProviderSetupModal;
