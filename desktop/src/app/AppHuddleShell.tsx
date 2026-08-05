import type * as React from "react";
import { AppHuddleBar } from "@/app/AppHuddleBar";
import * as MajuTheme from "@/app/MajuThemeSurfaces";
import { HuddleProvider } from "@/features/huddle";
import { RemindMeLaterProvider } from "@/features/reminders/ui/RemindMeLaterProvider";
import { cn } from "@/shared/lib/cn";

type AppHuddleShellProps = {
  children: React.ReactNode;
  currentPubkey?: string;
  isCompanionOpen: boolean;
  isDrawerOpen: boolean;
  isRoom: boolean;
  onCompanionOpen: () => void;
  onHuddleStartPendingChange: (pending: boolean) => void;
  onHuddleStarted: (ephemeralChannelId: string) => void | Promise<void>;
  onShowHuddleInMainApp: (ephemeralChannelId: string) => void;
  onViewHuddleChannel: (ephemeralChannelId: string) => void;
  onVisibilityChange: (visible: boolean) => void;
};

export function AppHuddleShell({
  children,
  currentPubkey,
  isCompanionOpen,
  isDrawerOpen,
  isRoom,
  onCompanionOpen,
  onHuddleStartPendingChange,
  onHuddleStarted,
  onShowHuddleInMainApp,
  onViewHuddleChannel,
  onVisibilityChange,
}: AppHuddleShellProps) {
  return (
    <HuddleProvider
      ownsAudioSession={!isRoom}
      onHuddleStartPendingChange={
        isRoom ? undefined : onHuddleStartPendingChange
      }
      onHuddleStarted={isRoom ? undefined : onHuddleStarted}
      onShowHuddleInMainApp={isRoom ? undefined : onShowHuddleInMainApp}
      onViewHuddleChannel={isRoom ? undefined : onViewHuddleChannel}
    >
      <RemindMeLaterProvider pubkey={currentPubkey}>
        <div
          className="maju-huddle-shell relative h-dvh overflow-hidden overscroll-none"
          data-huddle-open={isDrawerOpen}
          data-huddle-window={isRoom}
        >
          <div
            aria-hidden="true"
            className={cn(
              "maju-huddle-drawer-backdrop",
              isDrawerOpen && "maju-huddle-drawer-backdrop-open",
            )}
          />
          <div
            className={cn(
              "maju-huddle-app-surface z-10 flex min-h-0 flex-row overflow-hidden bg-background",
              isDrawerOpen &&
                (isRoom
                  ? "maju-huddle-app-surface-room-open"
                  : "maju-huddle-app-surface-open"),
            )}
          >
            <MajuTheme.GradientLayer />
            {children}
          </div>
          {isRoom || !isCompanionOpen ? (
            <div className="absolute inset-x-0 bottom-0 z-[2] h-(--maju-huddle-drawer-height)">
              <AppHuddleBar
                mode={isRoom ? "room" : "main"}
                onOpenHuddleWindow={isRoom ? undefined : onCompanionOpen}
                onVisibilityChange={onVisibilityChange}
              />
            </div>
          ) : null}
        </div>
      </RemindMeLaterProvider>
    </HuddleProvider>
  );
}
