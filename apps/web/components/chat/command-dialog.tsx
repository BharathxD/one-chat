"use client";

import { useAutoScroll } from "@/hooks/use-auto-scroll";
import { useQuery } from "@tanstack/react-query"; // Changed
import { apiClient, ApiError } from "@/lib/api-client"; // Changed
import type { ApiThread } from "@/components/thread/thread-list"; // Re-use type
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@workspace/ui/components/command";
import { ScrollArea } from "@workspace/ui/components/scroll-area";
import {
  Brain,
  ChevronRight,
  MessageSquare,
  Plus,
  Settings,
  Share2,
  Trash2,
} from "lucide-react";
import { useRouter } from "next/navigation";
import React, { useEffect } from "react";

interface CommandDialogComponentProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  threads: ApiThread[]; // Use ApiThread type
  isLoading: boolean;
}

// API call function (can be defined here or imported if shared)
const fetchUserThreads = async (): Promise<ApiThread[]> => {
  return apiClient.get<ApiThread[]>("/threads");
};

const CommandDialogComponent: React.FC<CommandDialogComponentProps> = ({
  open,
  onOpenChange,
  threads: initialThreads, // renamed to avoid conflict with query data
  isLoading: initialIsLoading,
}) => {
  const router = useRouter();
  const { scrollAreaRef } = useAutoScroll(initialThreads);

  // Fetch threads if not provided or for real-time updates (though staleTime is long)
  const { data: threads = initialThreads, isLoading = initialIsLoading } =
    useQuery<ApiThread[], ApiError>({
      queryKey: ["userThreads"], // Consistent query key
      queryFn: fetchUserThreads,
      staleTime: 300000, // 5 minutes
      // enabled: !initialThreads || initialThreads.length === 0, // Example: only fetch if no initial data
  });


  const runCommand = React.useCallback((command: () => unknown) => {
    onOpenChange(false);
    command();
  }, [onOpenChange]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: <explanation>
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        onOpenChange(!open);
      }
    };
    document.addEventListener("keydown", down);
    return () => document.removeEventListener("keydown", down);
  }, [onOpenChange]);

  return (
    <CommandDialog open={open} onOpenChange={onOpenChange}>
      <CommandInput placeholder="Type a command or search..." />
      <CommandList>
        <CommandEmpty>No results found.</CommandEmpty>
        <CommandGroup heading="Actions">
          <CommandItem onSelect={() => runCommand(() => router.push("/chat"))}>
            <Plus className="mr-2 size-4" />
            New Chat
          </CommandItem>
          <CommandItem
            onSelect={() => runCommand(() => router.push("/settings"))}
          >
            <Settings className="mr-2 size-4" />
            Settings
          </CommandItem>
          <CommandItem
            onSelect={() => runCommand(() => router.push("/settings/shares"))}
          >
            <Share2 className="mr-2 size-4" />
            Manage Shares
          </CommandItem>
          <CommandItem
            onSelect={() =>
              runCommand(() => router.push("/settings/model-settings"))
            }
          >
            <Brain className="mr-2 size-4" />
            Model Settings
          </CommandItem>
        </CommandGroup>
        <CommandSeparator />
        {isLoading ? (
          <CommandItem disabled>Loading threads...</CommandItem>
        ) : (
          <CommandGroup heading="Threads">
            <ScrollArea className="max-h-64" ref={scrollAreaRef}>
              {threads.map((thread) => (
                <CommandItem
                  key={thread.id}
                  value={thread.title}
                  onSelect={() =>
                    runCommand(() => router.push(`/chat/${thread.id}`))
                  }
                  className="group flex items-center justify-between"
                >
                  <div className="flex items-center">
                    <MessageSquare className="mr-2 size-4" />
                    <span className="truncate">{thread.title}</span>
                  </div>
                  <ChevronRight className="ml-auto size-4 opacity-0 transition-opacity duration-200 group-hover:opacity-100" />
                </CommandItem>
              ))}
            </ScrollArea>
          </CommandGroup>
        )}
        <CommandSeparator />
        <CommandGroup heading="Danger Zone">
          <CommandItem
            className="text-destructive focus:bg-destructive/10 focus:text-destructive dark:focus:bg-destructive/20"
            onSelect={() =>
              runCommand(() => router.push("/settings/delete-account"))
            }
          >
            <Trash2 className="mr-2 size-4" />
            Delete Account
          </CommandItem>
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  );
};

// Props for the main export, which might be different now or could be simplified
interface CommandDialogExportProps {
 open: boolean;
 onOpenChange: (open: boolean) => void;
 // The threads and isLoading props might not be needed if fetched internally always
 // threads: ApiThread[];
 // isLoading: boolean;
}


// This component now fetches its own thread data.
// The props `threads` and `isLoading` passed to CommandDialogComponent might be redundant
// if the internal useQuery is always active.
// For now, I've kept the structure where it can receive initialThreads,
// but the internal useQuery will likely be the primary source.
export const CommandDialogWrapper = ({ open, onOpenChange }: CommandDialogExportProps) => {
  // If CommandDialogComponent always fetches, these props for initial data are not strictly necessary
  // from the parent, unless for SSR or initial hydration without immediate fetch.
  // For simplicity now, assuming CommandDialogComponent handles its data.
  return <CommandDialogComponent open={open} onOpenChange={onOpenChange} threads={[]} isLoading={false} />;
};
