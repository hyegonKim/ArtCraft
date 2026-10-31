import { LoaderCircle, type LucideIcon } from "lucide-react";
import { twMerge } from "tailwind-merge";
import { ButtonHTMLAttributes, AnchorHTMLAttributes } from "react";

type AnchorProps = Omit<
  AnchorHTMLAttributes<HTMLAnchorElement>,
  keyof ButtonHTMLAttributes<HTMLButtonElement>
>;

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon?: LucideIcon;
  iconClassName?: string;
  iconFlip?: boolean;
  htmlFor?: string;
  variant?: "primary" | "secondary" | "action" | "destructive" | "ghost";
  loading?: boolean;
  as?: "button" | "link";
  href?: string;
  target?: string;
}

export const Button = ({
  icon: Icon,
  iconClassName,
  children,
  className: propsClassName,
  htmlFor,
  variant: propsVariant = "primary",
  disabled,
  iconFlip = false,
  loading,
  as = "button",
  href,
  target,
  ...rest
}: ButtonProps) => {
  // Marketing-site language: hairline-bordered pills with faint fills; the
  // one solid surface is the signal-blue primary.
  function getVariantClassNames(variant: string) {
    switch (variant) {
      case "secondary": {
        return "bg-bone/[0.04] text-putty border border-line-2 hover:bg-bone/[0.08] hover:text-bone focus-visible:outline-secondary";
      }
      case "action": {
        return "bg-bone/[0.04] text-putty border border-line-2 hover:bg-bone/[0.08] hover:text-bone focus-visible:outline-action";
      }
      case "destructive": {
        return "bg-red hover:bg-red/90 text-white focus-visible:outline-red";
      }
      case "ghost": {
        return "bg-transparent text-ash border border-transparent hover:bg-bone/[0.06] hover:text-bone focus-visible:outline-primary-600";
      }
      case "primary":
      default: {
        return "bg-primary hover:bg-primary-400 text-white focus-visible:outline-primary-600";
      }
    }
  }

  const disabledClass = twMerge(
    disabled || loading ? "opacity-50 pointer-events-none" : ""
  );

  const className = twMerge(
    "w-fit text-sm font-medium rounded-lg px-3 py-1.5 border border-transparent focus-visible:outline focus-visible:outline-0 focus-visible:outline-offset-0 transition-all duration-150 flex gap-2 items-center justify-center active:scale-95 transform",
    getVariantClassNames(propsVariant),
    propsClassName,
    disabledClass
  );

  if (htmlFor) {
    return (
      <label className={className} htmlFor={htmlFor} style={rest.style}>
        {loading && !iconFlip ? (
          <LoaderCircle size="1em" className="animate-spin" />
        ) : (
          <>
            {Icon && !iconFlip ? (
              <Icon size="1em" className={iconClassName} />
            ) : null}
          </>
        )}
        {children}
        {loading && iconFlip ? (
          <LoaderCircle size="1em" className="animate-spin" />
        ) : (
          <>
            {Icon && iconFlip ? (
              <Icon size="1em" className={iconClassName} />
            ) : null}
          </>
        )}
      </label>
    );
  }

  if (as === "link" && href) {
    return (
      <a
        href={href}
        className={className}
        style={rest.style}
        {...(rest as unknown as AnchorProps)}
        target={target}
      >
        {loading && !iconFlip ? (
          <LoaderCircle size="1em" className="animate-spin" />
        ) : (
          <>
            {Icon && !iconFlip ? (
              <Icon size="1em" className={iconClassName} />
            ) : null}
          </>
        )}
        {children}
        {loading && iconFlip ? (
          <LoaderCircle size="1em" className="animate-spin" />
        ) : (
          <>
            {Icon && iconFlip ? (
              <Icon size="1em" className={iconClassName} />
            ) : null}
          </>
        )}
      </a>
    );
  }

  return (
    <button
      className={className}
      disabled={disabled || loading}
      {...{ ...rest, htmlFor }}
    >
      {loading && !iconFlip ? (
        <LoaderCircle size="1em" className="animate-spin" />
      ) : (
        <>
          {Icon && !iconFlip ? (
            <Icon size="1em" className={iconClassName} />
          ) : null}
        </>
      )}
      {children}
      {loading && iconFlip ? (
        <LoaderCircle size="1em" className="animate-spin" />
      ) : (
        <>
          {Icon && iconFlip ? (
            <Icon size="1em" className={iconClassName} />
          ) : null}
        </>
      )}
    </button>
  );
};

export default Button;
