import * as React from "react";
import { Bot, Laptop, Monitor, Pencil, Unplug } from "lucide-react";
import { toast } from "sonner";

import { listManagedAgents } from "@/shared/api/tauri";
import {
  disconnectLoggedInDevice,
  listLoggedInDevices,
  renameCurrentDevice,
  type LoggedInDevice,
} from "@/shared/api/tauriDevices";
import { cn } from "@/shared/lib/cn";
import { truncatePubkey } from "@/shared/lib/pubkey";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/shared/ui/alert-dialog";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import { Spinner } from "@/shared/ui/spinner";
import { SettingsSectionHeader } from "./SettingsSectionHeader";

const REFRESH_MS = 15_000;

function platformLabel(platform: string): string {
  switch (platform) {
    case "windows":
      return "Windows";
    case "linux":
      return "Linux";
    case "macos":
      return "macOS";
    default:
      return platform;
  }
}

function lastSeenLabel(lastSeen: number, online: boolean): string {
  if (online) return "온라인";
  const elapsed = Math.max(0, Math.floor(Date.now() / 1_000) - lastSeen);
  if (elapsed < 60) return "방금 접속";
  if (elapsed < 3_600) return `${Math.floor(elapsed / 60)}분 전 접속`;
  if (elapsed < 86_400) return `${Math.floor(elapsed / 3_600)}시간 전 접속`;
  return `${Math.floor(elapsed / 86_400)}일 전 접속`;
}

export function DevicesSettingsCard() {
  const [devices, setDevices] = React.useState<LoggedInDevice[]>([]);
  const [agentNames, setAgentNames] = React.useState<Map<string, string>>(
    () => new Map(),
  );
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);
  const [renaming, setRenaming] = React.useState(false);
  const [name, setName] = React.useState("");
  const [savingName, setSavingName] = React.useState(false);
  const [disconnectTarget, setDisconnectTarget] =
    React.useState<LoggedInDevice | null>(null);
  const [disconnecting, setDisconnecting] = React.useState(false);

  const load = React.useCallback(async (showLoading = false) => {
    if (showLoading) setLoading(true);
    try {
      const [nextDevices, agents] = await Promise.all([
        listLoggedInDevices(),
        listManagedAgents(),
      ]);
      setDevices(nextDevices);
      setAgentNames(new Map(agents.map((agent) => [agent.pubkey, agent.name])));
      setError(null);
      const current = nextDevices.find((device) => device.current);
      if (current) setName(current.name);
    } catch (cause) {
      setError(
        cause instanceof Error
          ? cause.message
          : "기기 목록을 불러오지 못했어요.",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    void load(true);
    const timer = window.setInterval(() => void load(), REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [load]);

  async function saveName() {
    const trimmed = name.trim();
    if (!trimmed) return;
    setSavingName(true);
    try {
      await renameCurrentDevice(trimmed);
      await load();
      setRenaming(false);
      toast.success("기기 이름을 바꿨어요.");
    } catch (cause) {
      toast.error(
        cause instanceof Error ? cause.message : "이름을 바꾸지 못했어요.",
      );
    } finally {
      setSavingName(false);
    }
  }

  async function disconnectTargetDevice() {
    if (!disconnectTarget) return;
    setDisconnecting(true);
    try {
      await disconnectLoggedInDevice(disconnectTarget);
      toast.success(`${disconnectTarget.name}의 로그인을 해제했어요.`);
      setDisconnectTarget(null);
      await load();
    } catch (cause) {
      toast.error(
        cause instanceof Error
          ? cause.message
          : "기기 연결을 해제하지 못했어요.",
      );
    } finally {
      setDisconnecting(false);
    }
  }

  return (
    <section className="min-w-0" data-testid="settings-devices">
      <SettingsSectionHeader
        title="내 기기"
        description="같은 계정으로 로그인한 PC와 각 에이전트가 실제로 실행되는 위치를 확인해요."
      />

      {loading ? (
        <div className="flex min-h-40 items-center justify-center text-muted-foreground">
          <Spinner aria-label="기기 목록 불러오는 중" className="h-5 w-5" />
        </div>
      ) : error ? (
        <div className="rounded-xl border border-destructive/40 p-5">
          <p className="text-sm text-destructive">{error}</p>
          <Button
            className="mt-4"
            onClick={() => void load(true)}
            variant="outline"
          >
            다시 시도
          </Button>
        </div>
      ) : devices.length === 0 ? (
        <div className="rounded-xl border border-border p-6 text-center">
          <Laptop className="mx-auto h-7 w-7 text-muted-foreground" />
          <p className="mt-3 text-base font-medium">등록된 기기가 없어요</p>
          <p className="mt-1 text-sm text-muted-foreground">
            릴레이에 다시 연결하면 이 PC가 자동으로 등록돼요.
          </p>
        </div>
      ) : (
        <div className="divide-y divide-border overflow-hidden rounded-xl border border-border">
          {devices.map((device) => (
            <div
              className="flex min-w-0 flex-col gap-4 p-4 sm:flex-row sm:items-start sm:justify-between"
              data-testid={`device-row-${device.deviceId}`}
              key={`${device.deviceId}:${device.sessionId}`}
            >
              <div className="flex min-w-0 gap-3">
                <div className="relative mt-0.5 shrink-0 self-start">
                  <span className="flex h-10 w-10 items-center justify-center rounded-lg border border-border bg-background">
                    <Monitor className="h-5 w-5" />
                  </span>
                  <span
                    aria-label={device.online ? "온라인" : "오프라인"}
                    className={cn(
                      "absolute -bottom-0.5 -right-0.5 h-3 w-3 rounded-full border-2 border-background",
                      device.online
                        ? "bg-emerald-500"
                        : "bg-muted-foreground/50",
                    )}
                    role="img"
                  />
                </div>
                <div className="min-w-0">
                  <div className="flex min-w-0 flex-wrap items-center gap-2">
                    <p className="truncate text-base font-medium">
                      {device.name}
                    </p>
                    {device.current ? (
                      <Badge variant="secondary">이 기기</Badge>
                    ) : null}
                  </div>
                  <p className="mt-0.5 text-sm text-muted-foreground">
                    {platformLabel(device.platform)} · Maju {device.appVersion}{" "}
                    · {lastSeenLabel(device.lastSeen, device.online)}
                  </p>
                  {device.activeAgents.length > 0 ||
                  device.standbyAgents.length > 0 ? (
                    <div className="mt-3 flex flex-wrap gap-2">
                      {device.activeAgents.map((pubkey) => (
                        <Badge
                          className="gap-1"
                          key={`active:${pubkey}`}
                          variant="default"
                        >
                          <Bot className="h-3 w-3" />
                          대표 실행 ·{" "}
                          {agentNames.get(pubkey) ?? truncatePubkey(pubkey)}
                        </Badge>
                      ))}
                      {device.standbyAgents.map((pubkey) => (
                        <Badge
                          className="gap-1"
                          key={`standby:${pubkey}`}
                          variant="outline"
                        >
                          <Bot className="h-3 w-3" />
                          대기 ·{" "}
                          {agentNames.get(pubkey) ?? truncatePubkey(pubkey)}
                        </Badge>
                      ))}
                    </div>
                  ) : null}
                </div>
              </div>

              {device.current ? (
                renaming ? (
                  <form
                    className="flex w-full shrink-0 gap-2 sm:w-auto"
                    onSubmit={(event) => {
                      event.preventDefault();
                      void saveName();
                    }}
                  >
                    <Input
                      aria-label="기기 이름"
                      autoFocus
                      className="min-w-0 sm:w-44"
                      disabled={savingName}
                      maxLength={80}
                      onChange={(event) => setName(event.target.value)}
                      value={name}
                    />
                    <Button
                      disabled={savingName || !name.trim()}
                      size="sm"
                      type="submit"
                    >
                      저장
                    </Button>
                    <Button
                      disabled={savingName}
                      onClick={() => setRenaming(false)}
                      size="sm"
                      type="button"
                      variant="ghost"
                    >
                      취소
                    </Button>
                  </form>
                ) : (
                  <Button
                    className="self-start"
                    onClick={() => setRenaming(true)}
                    size="sm"
                    variant="ghost"
                  >
                    <Pencil className="h-4 w-4" />
                    이름 변경
                  </Button>
                )
              ) : (
                <Button
                  className="self-start"
                  onClick={() => setDisconnectTarget(device)}
                  size="sm"
                  variant="outline"
                >
                  <Unplug className="h-4 w-4" />
                  연결 해제
                </Button>
              )}
            </div>
          ))}
        </div>
      )}

      <p className="mt-4 text-sm text-muted-foreground">
        연결 해제는 해당 PC의 현재 로그인을 끝내요. 계정 개인키 자체는 폐기하지
        않아 다시 로그인할 수 있어요.
      </p>

      <AlertDialog
        onOpenChange={(open) => {
          if (!open && !disconnecting) setDisconnectTarget(null);
        }}
        open={disconnectTarget !== null}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {disconnectTarget?.name} 연결을 해제할까요?
            </AlertDialogTitle>
            <AlertDialogDescription>
              이 PC의 Maju가 로그아웃되고, 여기서 실행 중인 에이전트는 다른
              온라인 PC가 자동으로 이어받아요.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={disconnecting}>취소</AlertDialogCancel>
            <AlertDialogAction
              disabled={disconnecting}
              onClick={(event) => {
                event.preventDefault();
                void disconnectTargetDevice();
              }}
            >
              {disconnecting ? <Spinner className="h-4 w-4" /> : null}
              연결 해제
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
