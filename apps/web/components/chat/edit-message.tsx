"use client";

import { Button } from "@workspace/ui/components/button";
import { useMutation, useQueryClient } from "@tanstack/react-query"; // Changed
import { apiClient, ApiError } from "@/lib/api-client"; // Changed
import { toast } from "@workspace/ui/components/sonner";
import { RefreshCcw } from "lucide-react";
import type { Message } from "ai"; // Assuming this Message type is still relevant

interface EditMessageProps {
  message: Message; // This might need to be our ApiMessage type if structure differs
  threadId: string; // Pass threadId for cache invalidation
  // onEditSubmit: (newMessageContent: string) => void; // If inline editing was planned
  // For now, focusing on "regenerate from here" which means deleting trailing
}

// Define the expected shape of the deletion response from Axum API
interface DeletionResponse {
  deleted_count: number;
  message: string;
}


export const EditMessageActions: React.FC<EditMessageProps> = ({
  message,
  threadId,
}) => {
  const queryClient = useQueryClient();

  const deleteTrailingMessagesMutation = useMutation<
    DeletionResponse,
    ApiError,
    { messageId: string }
  >({
    mutationFn: async ({ messageId }) => {
      // Axum endpoint is POST /api/messages/:message_id/delete-trailing
      return apiClient.post(`/messages/${messageId}/delete-trailing`);
    },
    onSuccess: (data) => {
      toast.success(data.message || `${data.deleted_count} message(s) deleted.`);
      // Invalidate messages for the current thread to refetch
      queryClient.invalidateQueries({ queryKey: ["messages", threadId] });
      // Potentially invalidate other related queries if necessary
      // e.g., if thread list shows last message preview:
      queryClient.invalidateQueries({ queryKey: ["userThreads"] });
    },
    onError: (error) => {
      toast.error(`Failed to delete messages: ${error.message}`);
    },
  });

  const handleRegenerateFromHere = () => {
    if (window.confirm("This will delete all messages after this point. Continue?")) {
      deleteTrailingMessagesMutation.mutate({ messageId: message.id });
    }
  };

  // Only show for user messages or assistant messages that are not actively streaming/ errored
  // The original logic for when to show this might need review based on message structure
  const canRegenerate = message.role === "user" || (message.role === "assistant" /* && message.status === 'done' (if status exists) */) ;


  if (!canRegenerate || deleteTrailingMessagesMutation.isPending) {
    return null;
  }

  return (
    <div className="mt-1">
      <Button
        variant="ghost"
        size="sm"
        className="h-auto gap-1.5 px-2 py-1 text-xs text-muted-foreground hover:text-foreground"
        onClick={handleRegenerateFromHere}
        disabled={deleteTrailingMessagesMutation.isPending}
      >
        <RefreshCcw className="size-3" />
        Regenerate from here
      </Button>
    </div>
  );
};
