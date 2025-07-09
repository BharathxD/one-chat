"use client";

import { useMutation, useQueryClient } from "@tanstack/react-query"; // Changed
import { apiClient, ApiError } from "@/lib/api-client"; // Changed
import { Button } from "@workspace/ui/components/button";
import { toast } from "@workspace/ui/components/sonner";
import { Copy, Share2, Check } from "lucide-react";
import { useState } from "react";
import { nanoid } from "nanoid";
import type { Message } from "ai"; // Assuming this is still relevant for message prop
import { env } from "@/env";

interface MessageActionsProps {
  message: Message; // This might be our ApiMessage type
  threadId: string;
  // className?: string;
}

// Define the expected response shape for creating a partial share
export interface ApiPartialShareResponse {
  token: string;
  threadId: string;
  userId: string;
  sharedUpToMessageId: string; // Ensure camelCase matches Axum response if transformed
  createdAt: string;
}


export const MessageActions: React.FC<MessageActionsProps> = ({
  message,
  threadId,
}) => {
  const [copied, setCopied] = useState(false);
  const queryClient = useQueryClient();

  const handleCopy = () => {
    if (typeof message.content === "string") {
      navigator.clipboard.writeText(message.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } else {
      // Handle non-string content if necessary (e.g. complex UI content)
      toast.error("Cannot copy this message type.");
    }
  };

  const createShareLinkMutation = useMutation<
    ApiPartialShareResponse,
    ApiError,
    { threadId: string; messageId: string; token?: string }
  >({
    mutationFn: async ({ threadId, messageId, token }) => {
      return apiClient.post<ApiPartialShareResponse, { thread_id: string; shared_up_to_message_id: string; token?: string }>(
        "/shares",
        { thread_id: threadId, shared_up_to_message_id: messageId, token }
      );
    },
    onSuccess: (data) => {
      const shareUrl = `${env.NEXT_PUBLIC_APP_URL}/share/${data.token}`;
      navigator.clipboard.writeText(shareUrl);
      toast.success(`Share link copied to clipboard: ${shareUrl}`);
      queryClient.invalidateQueries({ queryKey: ["userPartialShares"] });
    },
    onError: (error) => {
      toast.error(`Failed to create share link: ${error.message}`);
    },
  });

  const handleShare = () => {
    createShareLinkMutation.mutate({
      threadId: threadId,
      messageId: message.id, // Assuming message.id is the correct ID for sharing up to
      token: nanoid(10), // Generate a client-side token suggestion
    });
  };

  // Show actions for assistant messages or user messages that are not empty.
  // (Original logic might have been more nuanced based on message types/status)
  const showActions = message.role !== "system" && message.content;

  if (!showActions) {
    return null;
  }

  return (
    <div className="mt-1 flex items-center gap-1">
      <Button
        variant="ghost"
        size="icon"
        className="size-7 text-muted-foreground transition-colors hover:text-foreground"
        onClick={handleCopy}
        aria-label="Copy message"
      >
        {copied ? (
          <Check className="size-3.5" />
        ) : (
          <Copy className="size-3.5" />
        )}
      </Button>
      {message.role === "assistant" && ( // Or based on other conditions if sharing user messages is allowed
        <Button
          variant="ghost"
          size="icon"
          className="size-7 text-muted-foreground transition-colors hover:text-foreground"
          onClick={handleShare}
          disabled={createShareLinkMutation.isPending}
          aria-label="Share thread up to this message"
        >
          {createShareLinkMutation.isPending ? (
             <span className="size-3 animate-spin rounded-full border-2 border-current border-t-transparent" />
          ) : (
            <Share2 className="size-3.5" />
          )}
        </Button>
      )}
      {/* Add other actions like Edit (from EditMessageActions) or Regenerate here if needed */}
    </div>
  );
};
