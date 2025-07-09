"use client";

import { useChatScrollAnchor } from "@/hooks/use-chat-scroll-anchor";
import { useEnterSubmit } from "@/hooks/use-enter-submit";
import { useMessages } from "@/hooks/use-messages"; // This hook will need refactoring if it uses tRPC
import { useSession } from "@/lib/auth/client";
import { nanoid } from "nanoid";
import { useRouter }_from "next/navigation";
import React, { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "@workspace/ui/components/sonner";
import { useMutation, useQueryClient } from "@tanstack/react-query"; // Changed
import { apiClient, ApiError } from "@/lib/api-client"; // Changed
import type { ApiThread } from "@/components/thread/thread-list"; // Changed
import { ChatInput } from "./chat-input";
import { ChatMessages } from "./chat-messages";
import { FileWithPreview } from "@/types";

interface ChatProps {
  threadId?: string;
  className?: string;
}

export const Chat: React.FC<ChatProps> = ({ threadId, className }) => {
  const router = useRouter();
  const queryClient = useQueryClient(); // Changed: useQueryClient

  const {
    messages,
    setMessages,
    userInput,
    setUserInput,
    isGenerating,
    setIsGenerating,
    stopGeneration,
    reloadMessage,
    appendMessage,
    removeMessage,
    isLoading: isLoadingMessages, // from useMessages
  } = useMessages({ threadId }); // This hook needs to be checked/refactored

  const { data: session } = useSession();
  const user = session?.user;

  const { formRef, onKeyDown } = useEnterSubmit(async () => {
    if (userInput.trim() || files.length > 0) {
      await handleSendMessage();
    }
  });
  const { messagesRef, scrollRef, visibilityRef, scrollToBottom } =
    useChatScrollAnchor(messages);

  const [files, setFiles] = useState<FileWithPreview[]>([]); // Assuming file handling is separate for now

  // Mutation for generating and updating thread title
  const generateTitleMutation = useMutation<
    ApiThread, // Axum endpoint returns the updated thread
    ApiError,
    { threadId: string; userQuery: string },
    { previousThreads?: ApiThread[] } // Context for optimistic updates
  >({
    mutationFn: async ({ threadId, userQuery }) => {
      return apiClient.post<ApiThread, { userQuery: string }>(
        `/threads/${threadId}/generate-title`,
        { userQuery }
      );
    },
    onMutate: async ({ threadId }) => {
      await queryClient.cancelQueries({ queryKey: ["userThreads"] });
      const previousThreads = queryClient.getQueryData<ApiThread[]>(["userThreads"]);
      queryClient.setQueryData<ApiThread[]>(["userThreads"], (oldThreads) =>
        oldThreads?.map((t) =>
          t.id === threadId ? { ...t, title: "Generating title..." } : t
        ) ?? []
      );
      return { previousThreads };
    },
    onSuccess: (updatedThread) => {
      toast.success("Thread title updated!");
      queryClient.setQueryData<ApiThread[]>(["userThreads"], (oldThreads) =>
        oldThreads?.map((t) =>
          t.id === updatedThread.id ? updatedThread : t
        ) ?? []
      );
      // If there's a query for the specific thread details, update it too
      queryClient.setQueryData<ApiThread>(["thread", updatedThread.id], updatedThread);
    },
    onError: (err, variables, context) => {
      if (context?.previousThreads) {
        queryClient.setQueryData<ApiThread[]>(["userThreads"], context.previousThreads);
      }
      toast.error(`Failed to generate title: ${err.message}`);
    },
    onSettled: (_data, _error, variables) => {
      queryClient.invalidateQueries({ queryKey: ["userThreads"] });
      if (variables?.threadId) {
        queryClient.invalidateQueries({ queryKey: ["thread", variables.threadId] });
      }
    },
  });


  const handleSendMessage = useCallback(async () => {
    if (!user) {
      toast.error("You must be logged in to send messages.");
      return;
    }
    if (isGenerating) return;

    const currentInput = userInput;
    const currentFiles = files; // Assuming files are handled by ChatInput and passed up
    setUserInput(""); // Clear input immediately
    setFiles([]); // Clear files immediately

    const userMessageId = nanoid();
    appendMessage({
      id: userMessageId,
      role: "user",
      content: currentInput,
      // attachments: currentFiles, // Assuming attachments are part of message structure
      createdAt: new Date(),
    });
    setIsGenerating(true);

    try {
      // This is where the actual call to send message to backend would go.
      // The original code used Vercel AI SDK's `useChat` which handles this.
      // We need to replicate sending the message to our Axum API
      // and then streaming the response.
      // This part requires significant changes as `useChat` from `ai/react`
      // is deeply integrated with Next.js Edge runtime and Vercel's AI SDK.
      // For now, I'll placeholder this. The `useMessages` hook will be central to this.

      // Placeholder for sending message to Axum and getting response
      // This would involve:
      // 1. POSTing to `/api/threads/:thread_id/messages` (if threadId exists)
      //    or POSTing to `/api/chat` (a new endpoint for starting new chats and getting back a threadId + first message)
      // 2. Handling a streaming response from Axum if using SSE or similar.

      // For now, simulate assistant response for UI testing
      // const assistantMessageId = nanoid();
      // appendMessage({
      //   id: assistantMessageId,
      //   role: "assistant",
      //   content: "Placeholder response from assistant...",
      //   createdAt: new Date(),
      // });


      // Title generation logic (if it's a new thread and first user message)
      if (!threadId && messages.length === 0 && currentInput.trim().length > 0) {
        // This implies a new thread was just created by the first message.
        // The backend would handle creating the thread and returning its ID.
        // Let's assume `handleSendMessage` in `useMessages` (once refactored)
        // returns the new threadId.
        // For now, this part is tricky without knowing the exact flow from useMessages.
        //
        // If a new thread ID is obtained after the first message is sent:
        // const newThreadId = "some-new-thread-id-from-backend";
        // router.push(`/chat/${newThreadId}`, { scroll: false }); // Navigate without scroll
        // generateTitleMutation.mutate({ threadId: newThreadId, userQuery: currentInput });
      }


    } catch (error: any) {
      toast.error(error.message || "An error occurred.");
      // Potentially remove the optimistic user message if send failed.
      removeMessage(userMessageId);
    } finally {
      setIsGenerating(false);
    }
  }, [
    user,
    userInput,
    files,
    isGenerating,
    appendMessage,
    removeMessage,
    setUserInput,
    setIsGenerating,
    threadId,
    messages.length, // For title generation condition
    // generateTitleMutation, // Already a stable function from useMutation
    // router // Stable
  ]);

  // Auto-generate title for new chats if threadId changes and it's a new thread.
  // This effect needs to be re-evaluated. Title generation should happen after the *first message* is sent.
  // The original logic in `threadRouter.ts` for `generateAndUpdateThreadTitle` was a mutation.
  // It was likely called after the first user message was processed and a thread was established.
  useEffect(() => {
    // This effect was originally tied to `messages` and `threadId`.
    // If `!threadId` (new chat) and `messages.length === 1` (first user message),
    // and `messages[0].role === 'user'`, then generate title.
    // This logic should be triggered *after* the first message is successfully sent
    // and a `threadId` for the new chat is available.
    // The `handleSendMessage` function is a more appropriate place to trigger this,
    // once the new threadId is known.

    // For now, commenting out, as it's complex without the full message sending logic.
    // if (!threadId && messages.length === 1 && messages[0].role === "user") {
    //   const firstUserMessage = messages[0].content;
    //   if (firstUserMessage && user?.id) {
    //     // This assumes a threadId is available immediately, which is not true for a new chat.
    //     // This mutate call would need the *new* threadId.
    //     // generateTitleMutation.mutate({ id: ???, userQuery: firstUserMessage });
    //   }
    // }
  }, [messages, threadId, user?.id, generateTitleMutation]);


  return (
    <div className={cn("flex h-[calc(100dvh)] flex-col", className)}>
      <ChatMessages
        ref={messagesRef}
        messages={messages}
        isLoading={isLoadingMessages}
        isGenerating={isGenerating}
        onStopGeneration={stopGeneration}
        onReloadMessage={reloadMessage}
        // Pass other necessary props
      />
      <div
        ref={scrollRef}
        className="w-fullitems-center sticky bottom-0 z-10 flex justify-center bg-background/30 pt-2 backdrop-blur-md dark:bg-background/50"
      >
        <div
          ref={visibilityRef}
          className="mx-auto w-full max-w-3xl p-2 sm:px-4"
        >
          <ChatInput
            // Pass threadId if needed by ChatInput for uploads etc.
            threadId={threadId}
            input={userInput}
            onInputChange={(e) =>
              setUserInput(typeof e === "string" ? e : e.target.value)
            }
            onSubmit={handleSendMessage}
            isLoading={isGenerating || isLoadingMessages}
          />
        </div>
      </div>
    </div>
  );
};
