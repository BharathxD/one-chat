"use client";

import { cn } from "@workspace/ui/lib/utils";
import { GripVertical, MessageSquare, Trash2 } from "lucide-react";
import Link from "next/link";
import React, { useState } from "react";
import { Button } from "@workspace/ui/components/button";
import { toast } from "@workspace/ui/components/sonner";
import { useMutation, useQueryClient } from "@tanstack/react-query"; // Changed
import { apiClient, ApiError } from "@/lib/api-client"; // Changed
import type { ApiThread } from "./thread-list"; // Import type from sibling

interface ThreadItemProps {
  thread: ApiThread; // Use the new ApiThread type
  isActive: boolean;
  onSelect: () => void;
}

export const ThreadItem = ({
  thread,
  isActive,
  onSelect,
}: ThreadItemProps) => {
  const [isHovered, setIsHovered] = useState(false);
  const queryClient = useQueryClient(); // Changed: useQueryClient

  const deleteThreadMutation = useMutation<
    void, // DELETE typically returns no content (204) or simple success
    ApiError,
    { threadId: string },
    { previousThreads?: ApiThread[] } // Context type for onMutate/onError
  >({
    mutationFn: async ({ threadId }) => {
      return apiClient.delete(`/threads/${threadId}`); // Axum API endpoint
    },
    onMutate: async ({ threadId }) => {
      // Cancel any outgoing refetches (so they don't overwrite our optimistic update)
      await queryClient.cancelQueries({ queryKey: ["userThreads"] });

      // Snapshot the previous value
      const previousThreads = queryClient.getQueryData<ApiThread[]>([
        "userThreads",
      ]);

      // Optimistically update to the new value
      queryClient.setQueryData<ApiThread[]>(["userThreads"], (oldThreads) =>
        oldThreads ? oldThreads.filter((t) => t.id !== threadId) : []
      );

      // Return a context object with the snapshotted value
      return { previousThreads };
    },
    onError: (err, variables, context) => {
      toast.error(`Failed to delete thread: ${err.message}`);
      // If the mutation fails, use the context returned from onMutate to roll back
      if (context?.previousThreads) {
        queryClient.setQueryData<ApiThread[]>(
          ["userThreads"],
          context.previousThreads
        );
      }
    },
    onSuccess: () => {
      toast.success("Thread deleted successfully.");
    },
    onSettled: () => {
      // Always refetch after error or success:
      queryClient.invalidateQueries({ queryKey: ["userThreads"] });
    },
  });

  const handleDelete = (e: React.MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation(); // Prevent Link navigation
    if (window.confirm("Are you sure you want to delete this thread?")) {
      deleteThreadMutation.mutate({ threadId: thread.id });
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLAnchorElement>) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onSelect();
    }
  };

  return (
    <Link
      href={`/chat/${thread.id}`}
      onClick={onSelect}
      onKeyDown={handleKeyDown}
      className={cn(
        "group flex cursor-pointer items-center justify-between gap-2 rounded-lg p-2.5 text-sm text-foreground hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring dark:hover:bg-accent/60",
        isActive && "bg-accent dark:bg-accent/60",
        deleteThreadMutation.isPending && "opacity-50 pointer-events-none" // Visual feedback during mutation
      )}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      aria-current={isActive ? "page" : undefined}
    >
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <MessageSquare className="size-4 shrink-0 text-muted-foreground" />
        <span className="min-w-0 flex-1 truncate" title={thread.title}>
          {thread.title}
        </span>
      </div>
      {isHovered && !isActive && (
        <Button
          variant="ghost"
          size="icon"
          className="size-6 shrink-0 hover:bg-destructive/15 hover:text-destructive dark:hover:bg-destructive/20"
          onClick={handleDelete}
          disabled={deleteThreadMutation.isPending}
          aria-label="Delete thread"
        >
          {deleteThreadMutation.isPending ? (
            <span className="size-3 animate-spin rounded-full border-2 border-current border-t-transparent" />
          ) : (
            <Trash2 className="size-3.5" />
          )}
        </Button>
      )}
      {isActive && (
        <GripVertical className="size-4 shrink-0 text-muted-foreground" />
      )}
    </Link>
  );
};
