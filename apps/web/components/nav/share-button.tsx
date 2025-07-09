"use client";

import { Button } from "@workspace/ui/components/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@workspace/ui/components/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@workspace/ui/components/select";
import { toast } from "@workspace/ui/components/sonner";
import { Globe, Link as LinkIcon, Loader2, Lock, Trash2, UserCheck, Users } from "lucide-react";
import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"; // Changed
import { apiClient, ApiError } from "@/lib/api-client"; // Changed
import type { ApiThread } from "@/components/thread/thread-list"; // Changed
import type { ApiPartialShareResponse } from "@/components/chat/message-actions"; // Changed (assuming type moved or accessible)
import { env } from "@/env";


interface ShareButtonProps {
  thread: ApiThread; // Use ApiThread
  children?: React.ReactNode;
}

// API call functions
const fetchUserPartialShares = async (): Promise<ApiPartialShareResponse[]> => {
  return apiClient.get<ApiPartialShareResponse[]>("/shares");
};

const deletePartialShareFn = async (token: string): Promise<void> => {
  return apiClient.delete(`/shares/${token}`);
};

const toggleThreadVisibilityFn = async (payload: {
  threadId: string;
  visibility: "public" | "private";
}): Promise<ApiThread> => {
  return apiClient.put<ApiThread, { visibility: "public" | "private" }>(
    `/threads/${payload.threadId}/visibility`,
    { visibility: payload.visibility }
  );
};


export const ShareButton: React.FC<ShareButtonProps> = ({
  thread,
  children,
}) => {
  const [open, setOpen] = useState(false);
  const queryClient = useQueryClient();

  const { data: allPartialShares, isLoading: isLoadingShares } = useQuery<
    ApiPartialShareResponse[],
    ApiError
  >({
    queryKey: ["userPartialShares"],
    queryFn: fetchUserPartialShares,
    enabled: open, // Only fetch when dialog is open
  });

  const currentThreadShares = useMemo(() => {
    return allPartialShares?.filter((share) => share.threadId === thread.id) || [];
  }, [allPartialShares, thread.id]);

  const isPublic = thread.visibility === "public";
  const shareLink = isPublic ? `${env.NEXT_PUBLIC_APP_URL}/share/thread/${thread.id}` : "";


  const deletePartialShareMutation = useMutation<void, ApiError, { token: string }>({
    mutationFn: ({ token }) => deletePartialShareFn(token),
    onSuccess: () => {
      toast.success("Share link deleted.");
      queryClient.invalidateQueries({ queryKey: ["userPartialShares"] });
    },
    onError: (error) => {
      toast.error(`Failed to delete share link: ${error.message}`);
    },
  });

  const toggleVisibilityMutation = useMutation<
    ApiThread,
    ApiError,
    { threadId: string; visibility: "public" | "private" }
  >({
    mutationFn: toggleThreadVisibilityFn,
    onSuccess: (updatedThread) => {
      toast.success(`Thread visibility updated to ${updatedThread.visibility}.`);
      // Update the specific thread in cache
      queryClient.setQueryData<ApiThread>(["thread", updatedThread.id], updatedThread);
      // Update the thread in the userThreads list
      queryClient.setQueryData<ApiThread[]>(["userThreads"], (oldThreads) =>
        oldThreads?.map((t) => (t.id === updatedThread.id ? updatedThread : t))
      );
      // Optionally, just invalidate to refetch everything
      // queryClient.invalidateQueries({ queryKey: ["userThreads"] });
      // queryClient.invalidateQueries({ queryKey: ["thread", updatedThread.id] });
    },
    onError: (error) => {
      toast.error(`Failed to update visibility: ${error.message}`);
    },
  });


  const handleVisibilityChange = (newVisibility: "public" | "private") => {
    if (thread.visibility === newVisibility) return;
    toggleVisibilityMutation.mutate({ threadId: thread.id, visibility: newVisibility });
  };

  const handleCopyLink = (linkToCopy: string) => {
    if (!linkToCopy) {
      toast.error("No link available to copy.");
      return;
    }
    navigator.clipboard.writeText(linkToCopy);
    toast.success("Link copied to clipboard!");
  };


  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>{children}</DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Share "{thread.title}"</DialogTitle>
          <DialogDescription>
            {isPublic
              ? "This thread is public. Anyone with the link can view it."
              : "Manage visibility or create specific share links for parts of this thread."}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <div>
            <label htmlFor="visibility" className="text-sm font-medium">
              Thread Visibility
            </label>
            <Select
              value={thread.visibility}
              onValueChange={(value: "public" | "private") => handleVisibilityChange(value)}
              disabled={toggleVisibilityMutation.isPending}
            >
              <SelectTrigger id="visibility" className="mt-1 w-full">
                <SelectValue placeholder="Select visibility" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="private">
                  <div className="flex items-center gap-2">
                    <Lock className="size-4" /> Private (Only you can see)
                  </div>
                </SelectItem>
                <SelectItem value="public">
                  <div className="flex items-center gap-2">
                    <Globe className="size-4" /> Public (Anyone with link)
                  </div>
                </SelectItem>
              </SelectContent>
            </Select>
            {toggleVisibilityMutation.isPending && <Loader2 className="mt-1 size-4 animate-spin" />}
          </div>

          {isPublic && shareLink && (
            <div className="space-y-2 rounded-md border p-3">
              <p className="text-sm font-medium">Public Link (Full Thread)</p>
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  readOnly
                  value={shareLink}
                  className="flex-1 rounded-md border bg-muted px-2 py-1.5 text-sm text-muted-foreground focus-visible:outline-none"
                />
                <Button variant="outline" size="icon" onClick={() => handleCopyLink(shareLink)}>
                  <LinkIcon className="size-4" />
                </Button>
              </div>
            </div>
          )}

          <div className="space-y-2">
            <p className="text-sm font-medium">Partial Share Links (Up to a message)</p>
            {isLoadingShares && <p className="text-xs text-muted-foreground">Loading share links...</p>}
            {currentThreadShares.length === 0 && !isLoadingShares && (
              <p className="text-xs text-muted-foreground">
                No partial share links created for this thread yet. You can create them from message actions in the chat.
              </p>
            )}
            <div className="max-h-32 space-y-1.5 overflow-y-auto">
              {currentThreadShares.map((share) => (
                <div key={share.token} className="flex items-center justify-between gap-2 rounded-md border p-2 text-sm">
                  <span className="truncate text-muted-foreground">
                    Shared up to message ID: ...{share.sharedUpToMessageId.slice(-6)}
                  </span>
                  <div className="flex items-center gap-1">
                    <Button variant="ghost" size="icon" className="size-7" onClick={() => handleCopyLink(`${env.NEXT_PUBLIC_APP_URL}/share/${share.token}`)}>
                      <LinkIcon className="size-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-7 hover:bg-destructive/15 hover:text-destructive"
                      onClick={() => deletePartialShareMutation.mutate({ token: share.token })}
                      disabled={deletePartialShareMutation.isPending && deletePartialShareMutation.variables?.token === share.token}
                    >
                      {deletePartialShareMutation.isPending && deletePartialShareMutation.variables?.token === share.token ? (
                        <Loader2 className="size-3.5 animate-spin" />
                      ) : (
                        <Trash2 className="size-3.5" />
                      )}
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => setOpen(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};
