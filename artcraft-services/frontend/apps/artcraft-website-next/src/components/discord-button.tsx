import { DiscordIcon } from "@/components/icons";
import { Button, type ButtonProps } from "@/components/ui";
import { SOCIAL_LINKS } from "@/lib/links";

// The site-wide "come talk to us" CTA: always the ArtCraft Discord, always
// a new tab.
export default function DiscordButton({
  children = "Join Discord",
  ...rest
}: Omit<ButtonProps, "href" | "target" | "rel" | "external">) {
  return (
    <Button
      href={SOCIAL_LINKS.DISCORD}
      target="_blank"
      rel="noopener noreferrer"
      {...rest}
    >
      <DiscordIcon className="h-4 w-4" />
      {children}
    </Button>
  );
}
