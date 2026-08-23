import type { CreateProjectInput } from "@/features/projects/useCreateProject";
import { CreateProjectDialog } from "@/features/projects/ui/CreateProjectDialog";
import { EmptyState } from "@/features/projects/ui/ProjectCards";
import { topChromeInset } from "@/shared/layout/chromeLayout";
import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/button";
import { PageHeader } from "@/shared/ui/PageHeader";
import { ViewLoadingFallback } from "@/shared/ui/ViewLoadingFallback";

export function ProjectsLoadingState() {
  return <ViewLoadingFallback kind="projects" />;
}

export function ProjectsLoadErrorState({ onRetry }: { onRetry: () => void }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-2 text-muted-foreground">
      <p className="text-sm text-red-400">Failed to load projects</p>
      <Button onClick={onRetry} size="sm" variant="outline">
        Retry
      </Button>
    </div>
  );
}

export function ProjectsEmptyState({
  isCreating,
  onCreate,
  onOpenChange,
  open,
}: {
  isCreating: boolean;
  onCreate: (input: CreateProjectInput) => Promise<void>;
  onOpenChange: (open: boolean) => void;
  open: boolean;
}) {
  return (
    <div
      className={cn(
        "relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-tl-xl",
        topChromeInset.divider,
      )}
    >
      <CreateProjectDialog
        isCreating={isCreating}
        onCreate={onCreate}
        onOpenChange={onOpenChange}
        open={open}
      />
      <div className="maju-content-scrollbar min-h-0 min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto flex min-h-full w-full max-w-6xl flex-col px-4 pb-8 pt-7 sm:px-6 sm:pt-8">
          <PageHeader
            description="Set up and manage your projects."
            title="Projects"
          />
          <EmptyState onCreate={() => onOpenChange(true)} />
        </div>
      </div>
    </div>
  );
}
