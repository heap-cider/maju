import { Search } from "lucide-react";

import type { ProjectsFilter } from "@/features/projects/lib/projectsViewHelpers";
import { PROJECT_COLUMN_HEADER_BACKDROP_CLASS } from "@/features/projects/ui/projectPanelStyles";
import { ProjectsToolbar } from "@/features/projects/ui/ProjectsToolbar";
import { openAppSearch } from "@/features/projects/ui/projectsSectionMeta";
import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/button";

export function ProjectsOverviewToolbarRow({
  detached,
  filter,
  onFilterChange,
}: {
  detached: boolean;
  filter: ProjectsFilter;
  onFilterChange: (filter: ProjectsFilter) => void;
}) {
  return (
    <div
      className={cn(
        "sticky top-0 z-30 -mx-4 flex h-13 min-w-0 items-center gap-1.5 px-4",
        PROJECT_COLUMN_HEADER_BACKDROP_CLASS,
        detached && "rounded-t-2xl",
      )}
      data-testid="projects-page-tabs"
    >
      <Button
        aria-label="Search everything"
        className="h-7 w-7 shrink-0 rounded-full border border-border/55 bg-transparent text-muted-foreground shadow-none hover:border-border hover:bg-muted/25 hover:text-foreground focus-visible:border-border"
        data-testid="projects-activity-search"
        onClick={openAppSearch}
        size="icon"
        title="Search everything"
        type="button"
        variant="ghost"
      >
        <Search className="h-4 w-4" />
      </Button>
      <div className="min-w-0 flex-1">
        <ProjectsToolbar filter={filter} onFilterChange={onFilterChange} />
      </div>
    </div>
  );
}
