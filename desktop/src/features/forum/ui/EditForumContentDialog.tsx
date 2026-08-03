import * as React from "react";

import { Button } from "@/shared/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/shared/ui/dialog";
import { Textarea } from "@/shared/ui/textarea";

type EditForumContentDialogProps = {
  content: string;
  isSaving: boolean;
  label: "post" | "reply";
  onOpenChange: (open: boolean) => void;
  onSave: (content: string) => Promise<void>;
  open: boolean;
};

export function EditForumContentDialog({
  content,
  isSaving,
  label,
  onOpenChange,
  onSave,
  open,
}: EditForumContentDialogProps) {
  const [draft, setDraft] = React.useState(content);
  const [error, setError] = React.useState<string | null>(null);
  const editorRef = React.useRef<HTMLTextAreaElement>(null);

  React.useEffect(() => {
    if (open) {
      setDraft(content);
      setError(null);
    }
  }, [content, open]);

  const save = async () => {
    const trimmed = draft.trim();
    if (!trimmed || trimmed === content.trim()) return;
    try {
      await onSave(trimmed);
      onOpenChange(false);
    } catch (saveError) {
      setError(
        saveError instanceof Error
          ? saveError.message
          : `Failed to edit ${label}`,
      );
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="max-w-xl"
        data-testid="edit-forum-dialog"
        onOpenAutoFocus={(event) => {
          event.preventDefault();
          editorRef.current?.focus({ preventScroll: true });
        }}
      >
        <DialogHeader>
          <DialogTitle>Edit {label}</DialogTitle>
          <DialogDescription>
            The edit keeps its original author and records who changed it.
          </DialogDescription>
        </DialogHeader>
        <Textarea
          className="min-h-40 resize-y"
          data-testid="edit-forum-content"
          disabled={isSaving}
          onChange={(event) => {
            setDraft(event.target.value);
            setError(null);
          }}
          ref={editorRef}
          value={draft}
        />
        {error ? <p className="text-sm text-destructive">{error}</p> : null}
        <DialogFooter>
          <Button
            disabled={isSaving}
            onClick={() => onOpenChange(false)}
            type="button"
            variant="ghost"
          >
            Cancel
          </Button>
          <Button
            data-testid="save-forum-edit"
            disabled={
              isSaving || !draft.trim() || draft.trim() === content.trim()
            }
            onClick={() => void save()}
            type="button"
          >
            {isSaving ? "Saving..." : "Save changes"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
