import * as React from "react";
import { Check, Copy } from "lucide-react";

import { useCommunityOnboarding } from "@/features/onboarding/communityOnboarding";
import { InviteRedeemForm } from "@/features/onboarding/ui/InviteRedeemForm";
import { OnboardingChrome } from "@/features/onboarding/ui/OnboardingChrome";
import { OnboardingFooterProvider } from "@/features/onboarding/ui/OnboardingFooter";
import { useIdentityQuery } from "@/shared/api/hooks";
import { writeTextToClipboard } from "@/shared/lib/clipboard";
import { pubkeyToNpub } from "@/shared/lib/nostrUtils";
import { useSystemColorScheme } from "@/shared/theme/useSystemColorScheme";
import { Button } from "@/shared/ui/button";
import { StartupWindowDragRegion } from "@/shared/ui/StartupWindowDragRegion";

type WelcomeSetupProps = {
  onBack?: () => void;
};

export function WelcomeSetup({ onBack }: WelcomeSetupProps) {
  const [copiedNpub, setCopiedNpub] = React.useState(false);
  const [showPublicId, setShowPublicId] = React.useState(false);
  const communityOnboarding = useCommunityOnboarding();
  const identityQuery = useIdentityQuery();
  const systemColorScheme = useSystemColorScheme();
  const npub = identityQuery.data?.pubkey
    ? pubkeyToNpub(identityQuery.data.pubkey)
    : "";
  const npubError = identityQuery.error
    ? identityQuery.error instanceof Error
      ? identityQuery.error.message
      : "Could not load your public ID."
    : null;

  const startConnection = React.useCallback(
    (relayUrl: string, token?: string) => {
      communityOnboarding.start({
        source: "first-community",
        firstCommunityPage: "join",
        relayUrl,
        token,
      });
    },
    [communityOnboarding],
  );

  const redeemInvite = React.useCallback(
    (relayUrl: string, code: string, policyReceipt?: string) => {
      communityOnboarding.start({
        source: "first-community",
        firstCommunityPage: "join",
        relayUrl,
        inviteCode: code,
        policyReceipt,
      });
    },
    [communityOnboarding],
  );

  return (
    <div
      className="maju-onboarding-neutral-theme maju-startup-shell flex h-dvh items-start justify-center overflow-y-auto bg-background px-4 pb-36 pt-[106px] text-foreground"
      data-system-color-scheme={systemColorScheme}
      data-testid="self-hosted-welcome-setup"
    >
      <StartupWindowDragRegion />
      <OnboardingChrome current={5} />
      <OnboardingFooterProvider>
        <div className="relative flex min-h-0 w-full max-w-[920px] flex-1 flex-col items-center text-center">
          <div className="w-full max-w-[680px]">
            <h1 className="text-title font-normal">Connect your Maju server</h1>
            <p className="mt-3 text-sm leading-6 text-foreground/80">
              Enter the address of the relay you self-host, or paste an invite
              link. Maju uses the identity you just set up to authenticate.
            </p>
          </div>

          <div className="flex w-full flex-1 flex-col items-center justify-center gap-14 py-8">
            <InviteRedeemForm
              error={null}
              isRedeeming={false}
              onCancel={onBack}
              onConnect={startConnection}
              onRedeem={redeemInvite}
              placeholder="https://maju.example.com or paste an invite link"
              variant="onboarding-spotlight"
            />

            <div className="w-full max-w-[560px]">
              <button
                aria-expanded={showPublicId}
                className="text-sm text-foreground/70 underline decoration-foreground/25 underline-offset-4 transition-colors hover:text-foreground focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-foreground/35"
                data-testid="welcome-allowlist-reveal"
                onClick={() => setShowPublicId((visible) => !visible)}
                type="button"
              >
                Need to allow this account on your server first?
              </button>

              {showPublicId ? (
                <div
                  className="mt-5 text-left"
                  data-testid="welcome-allowlist-details"
                >
                  <p className="text-sm leading-6 text-foreground/75">
                    Add this public ID to the server allowlist. It is safe to
                    share and does not expose your private key.
                  </p>
                  <div className="mt-3 flex items-center gap-3 rounded-xl border border-foreground/10 bg-background/35 px-4 py-3">
                    <code
                      className="min-w-0 flex-1 truncate font-mono text-xs text-foreground/80"
                      data-testid="welcome-join-npub"
                    >
                      {npub || "Loading..."}
                    </code>
                    <Button
                      aria-label="Copy public ID"
                      className="h-9 shrink-0 rounded-full px-3"
                      disabled={!npub}
                      onClick={() => {
                        void writeTextToClipboard(npub).then(() => {
                          setCopiedNpub(true);
                          window.setTimeout(() => setCopiedNpub(false), 1500);
                        });
                      }}
                      size="sm"
                      type="button"
                      variant="outline"
                    >
                      {copiedNpub ? (
                        <Check className="h-4 w-4" aria-hidden="true" />
                      ) : (
                        <Copy className="h-4 w-4" aria-hidden="true" />
                      )}
                      <span>{copiedNpub ? "Copied" : "Copy"}</span>
                    </Button>
                  </div>
                  {npubError ? (
                    <p className="mt-3 text-sm text-destructive">{npubError}</p>
                  ) : null}
                </div>
              ) : null}
            </div>
          </div>
        </div>
      </OnboardingFooterProvider>
    </div>
  );
}
