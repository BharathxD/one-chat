"use client";

import { useAutoScroll } from "@/hooks/use-auto-scroll";
import { groupThreadsByTime } from "@/lib/utils/thread-grouping";
import { ScrollArea } from "@workspace/ui/components/scroll-area";
import { useQuery } from "@tanstack/react-query"; // Changed
import { apiClient, ApiError } from "@/lib/api-client"; // Changed
import { ThreadItem } from "./thread-item";
import { ThreadListSkeleton } from "./thread-list-skeleton";
import { useThreadNavigation } from "./use-thread-navigation";

// Define the expected shape of the thread response from Axum API
// This should ideally match the ThreadResponse struct from the Axum API
export interface ApiThread {
  id: string;
  userId: string;
  title: string;
  visibility: "private" | "public"; // Match Rust enum variants
  originThreadId?: string | null; // Ensure Option<String> maps correctly
  createdAt: string; // ISO 8601 string
  updatedAt: string; // ISO 8601 string
}

// API call function
const fetchUserThreads = async (): Promise<ApiThread[]> => {
  return apiClient.get<ApiThread[]>("/threads"); // Axum API endpoint
};

export const ThreadList = () => {
  const { data: threads = [], isLoading, isError, error } = useQuery<
    ApiThread[],
    ApiError
  >({
    queryKey: ["userThreads"], // React Query key for user's threads
    queryFn: fetchUserThreads,
    refetchOnWindowFocus: false, // Original setting
    staleTime: 300000, // 5 minutes, original setting
  });

  const { activeThreadId, navigateToThread } = useThreadNavigation(threads);
  const { scrollAreaRef, scrollToBottom, scrollToTop } = useAutoScroll(threads);
  const groupedThreads = groupThreadsByTime(threads);

  if (isLoading) {
    return <ThreadListSkeleton />;
  }

  if (isError) {
    return (
      <div className="p-4 text-sm text-red-500">
        Error loading threads: {error?.message || "Unknown error"}
      </div>
    );
  }

  if (threads.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-4">
        <p className="text-center text-sm text-muted-foreground">
          No threads yet. <br />
          Start a new conversation to see it here.
        </p>
      </div>
    );
  }

  return (
    <ScrollArea className="h-full" ref={scrollAreaRef}>
      <div className="space-y-1 p-2">
        {Object.entries(groupedThreads).map(([groupTitle, groupThreads]) => (
          <div key={groupTitle} className="mb-3">
            <h3 className="mb-1.5 px-2.5 text-xs font-semibold text-muted-foreground">
              {groupTitle}
            </h3>
            <div className="space-y-1">
              {groupThreads.map((thread) => (
                <ThreadItem
                  key={thread.id}
                  thread={thread}
                  isActive={activeThreadId === thread.id}
                  onSelect={() => navigateToThread(thread.id)}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
      {/* Optional: Add buttons for scroll to top/bottom if needed based on useAutoScroll */}
    </ScrollArea>
  );
};
