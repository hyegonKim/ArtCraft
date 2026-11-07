import { GoogleLogin, CredentialResponse } from "@react-oauth/google";
import { UsersApi } from "@storyteller/api";
import {
  getLandingUrl,
  getReferralCode,
  getReferralUsername,
  getReferrer,
} from "@storyteller/common";
import { LoaderCircleIcon } from "lucide-react";
import { useState } from "react";

interface GoogleLoginButtonProps {
  onSuccess: (isNewUser: boolean) => void;
  onError: (error: string) => void;
  mode?: "login" | "signup";
  className?: string;
}

export function GoogleLoginButton({
  onSuccess,
  onError,
  className = "",
}: GoogleLoginButtonProps) {
  const [isLoading, setIsLoading] = useState(false);

  const handleGoogleSuccess = async (
    credentialResponse: CredentialResponse,
  ) => {
    if (!credentialResponse.credential) {
      onError("No credential received from Google");
      return;
    }

    setIsLoading(true);
    try {
      const api = new UsersApi();
      const result = await api.GoogleSSO({
        credential: credentialResponse.credential,
        maybeReferralUrl: getReferrer(),
        maybeLandingUrl: getLandingUrl(),
        maybeReferralUsername: getReferralUsername(),
        maybeReferralCode: getReferralCode(),
      });
      if (result.success) {
        window.dispatchEvent(new Event("auth-change"));
        onSuccess(result.data?.usernameNotYetCustomized ?? false);
      } else {
        onError(result.errorMessage || "Authentication failed");
      }
    } catch (err) {
      onError("Network error");
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div
      className={`flex min-h-10 w-full items-center justify-center ${className}`}
      aria-busy={isLoading}
    >
      {isLoading ? (
        <LoaderCircleIcon className="animate-spin" aria-label="Signing in with Google" />
      ) : (
        <GoogleLogin
          onSuccess={handleGoogleSuccess}
          onError={() => onError("Google login failed")}
          size="large"
          text="continue_with"
        />
      )}
    </div>
  );
}
