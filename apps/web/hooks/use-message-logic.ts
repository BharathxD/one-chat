import { useSession } from "@/lib/auth/client";
import type { Message as AiSDKMessage } from "ai"; // This is from Vercel AI SDK
import { useMutation, useQueryClient } from "@tanstack/react-query"; // Changed
import { apiClient, ApiError } from "@/lib/api-client"; // Changed
import type { ApiThread } from "@/components/thread/thread-list"; // Changed
// Define response for delete operations if not already globally available
interface DeletionResponse {
  deleted_count: number;
  message: string;
}


import { nanoid } from "nanoid";
import { useRouter } from "next/navigation";
import { toast } from "@workspace/ui/components/sonner";

interface UseMessageLogicProps {
  threadId?: string; // Current thread ID
  // Add other props if the hook needs them, e.g., current messages list for context
}

export const useMessageLogic = ({ threadId }: UseMessageLogicProps) => {
  const router = useRouter();
  const queryClient = useQueryClient();
  const { data: session } = useSession();
  const user = session?.user;

  // Mutation for deleting a message and its trailing messages
  const deleteMessageAndTrailingMutation = useMutation<
    DeletionResponse, // Assuming this is the response from Axum
    ApiError,
    { messageId: string }
  >({
    mutationFn: async ({ messageId }) => {
      return apiClient.post<DeletionResponse>(
        `/messages/${messageId}/delete-inclusive-trailing`
      );
    },
    onSuccess: (data) => {
      toast.success(data.message || `${data.deleted_count} message(s) removed.`);
      if (threadId) {
        queryClient.invalidateQueries({ queryKey: ["messages", threadId] });
      }
      queryClient.invalidateQueries({ queryKey: ["userThreads"] });
    },
    onError: (error) => {
      toast.error(`Failed to remove message(s): ${error.message}`);
    },
  });

  // Mutation for branching out a new thread
  const branchOutMutation = useMutation<
    ApiThread, // Axum returns the new thread
    ApiError,
    { originalThreadId: string; anchorMessageId: string; newThreadId?: string },
    { previousThreads?: ApiThread[] } // Context for optimistic update
  >({
    mutationFn: async ({ originalThreadId, anchorMessageId, newThreadId }) => {
      return apiClient.post<ApiThread, { anchor_message_id: string; new_thread_id?: string }>(
        `/threads/${originalThreadId}/branch`,
        { anchor_message_id: anchorMessageId, new_thread_id: newThreadId }
      );
    },
    onMutate: async (variables) => {
      // Optimistically add a placeholder or navigate immediately then update.
      // For simplicity, we can just invalidate and let react-query handle refetch.
      // Or, more advanced:
      await queryClient.cancelQueries({ queryKey: ["userThreads"] });
      const previousThreads = queryClient.getQueryData<ApiThread[]>(["userThreads"]);

      // Add a temporary new thread optimistically (optional, can be complex)
      // For now, we'll rely on onSuccess navigation and invalidation.
      return { previousThreads };
    },
    onSuccess: (newThread) => {
      toast.success(`Branched into new thread: ${newThread.title}`);
      queryClient.invalidateQueries({ queryKey: ["userThreads"] });
      // Navigate to the new thread
      router.push(`/chat/${newThread.id}`);
    },
    onError: (error, variables, context) => {
      if (context?.previousThreads) {
        queryClient.setQueryData<ApiThread[]>(["userThreads"], context.previousThreads);
      }
      toast.error(`Failed to branch thread: ${error.message}`);
    },
    // onSettled: () => { // Already handled by onSuccess invalidation
    //   queryClient.invalidateQueries({ queryKey: ["userThreads"] });
    // },
  });


  const handleDeleteMessageAndFollowing = (message: AiSDKMessage) => {
    if (!user) return;
    if (window.confirm("Are you sure you want to delete this message and all following messages?")) {
      deleteMessageAndTrailingMutation.mutate({ messageId: message.id });
    }
  };

  const handleBranchOut = (message: AiSDKMessage) => {
    if (!user || !threadId) return; // Must have an original threadId to branch from

    const newThreadIdClient = nanoid(); // Generate a client-side ID for the new thread (can be suggestion)
    branchOutMutation.mutate({
      originalThreadId: threadId,
      anchorMessageId: message.id,
      newThreadId: newThreadIdClient,
    });
  };

  return {
    handleDeleteMessageAndFollowing,
    isLoadingDelete: deleteMessageAndTrailingMutation.isPending,
    handleBranchOut,
    isLoadingBranchOut: branchOutMutation.isPending,
  };
};
