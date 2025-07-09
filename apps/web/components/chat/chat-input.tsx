"use client";

import { useEnterSubmit } from "@/hooks/use-enter-submit";
import { cn } from "@workspace/ui/lib/utils";
import React, { useRef, useState } from "react";
import Textarea from "react-textarea-autosize";
import { useMutation, useQueryClient } from "@tanstack/react-query"; // Changed
import { apiClient, ApiError } from "@/lib/api-client"; // Changed
import { Button } from "@workspace/ui/components/button";
import { toast } from "@workspace/ui/components/sonner";
import {
  Paperclip,
  SendHorizontal,
  Trash2,
  Mic,
  Loader2,
  X,
} from "lucide-react";
import { FileWithPreview } from "@/types"; // Assuming this type is still relevant
import { useFileHandler } from "@/hooks/use-file-handler";
import Image from "next/image";
import { useVoiceInput } from "@/hooks/use-voice-input";

interface ChatInputProps {
  input: string;
  onInputChange: (
    e: React.ChangeEvent<HTMLTextAreaElement> | string
  ) => void;
  onSubmit: (value: string, attachments?: FileWithPreview[]) => Promise<void>;
  isLoading: boolean;
  threadId?: string;
  className?: string;
}

export const ChatInput: React.FC<ChatInputProps> = ({
  input,
  onInputChange,
  onSubmit,
  isLoading,
  // threadId, // threadId might be needed if file uploads are associated with a thread
  className,
}) => {
  const { formRef, onKeyDown } = useEnterSubmit(async () => {
    if (input.trim() || files.length > 0) {
      await handleSubmit();
    }
  });
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const [isFocused, setIsFocused] = useState(false);

  const {
    files,
    handleFileChange,
    removeFile,
    MAX_FILES,
    MAX_FILE_SIZE_MB,
  } = useFileHandler();

  const queryClient = useQueryClient(); // For potential cache invalidations

  // Refactored deleteAttachment mutation
  const deleteAttachmentMutation = useMutation<
    void,
    ApiError,
    { url: string }
  >({
    mutationFn: async ({ url }) => {
      // Axum endpoint is POST /api/attachments/delete with body { url: string }
      return apiClient.post("/attachments/delete", { url });
    },
    onSuccess: () => {
      toast.success("Attachment removed from storage.");
      // If attachment URLs are stored in message data, invalidate relevant message/thread queries
      // For example: queryClient.invalidateQueries({ queryKey: ["messages", threadId] });
    },
    onError: (error) => {
      toast.error(`Failed to remove attachment from storage: ${error.message}`);
    },
  });


  const handleRemoveFile = async (fileToRemove: FileWithPreview) => {
    // If the file has a `url` property, it means it was uploaded and might exist in blob storage
    if (fileToRemove.url) {
      try {
        await deleteAttachmentMutation.mutateAsync({ url: fileToRemove.url });
        // If successful, then remove from local state
        removeFile(fileToRemove);
      } catch (error) {
        // Error toast is handled by the mutation's onError
      }
    } else {
      // If no URL, it's a local preview, just remove from local state
      removeFile(fileToRemove);
    }
  };


  const handleSubmit = async () => {
    // Filter out files that are just previews (no URL yet) if onSubmit expects only uploaded files.
    // Or, the onSubmit function itself should handle the upload process if not already done.
    // For now, assuming `files` contains files that are ready to be associated with the message.
    // The actual upload logic might need to happen here or within onSubmit.
    // The original `onSubmit` took `attachments?: FileWithPreview[]`.
    // Let's assume `FileWithPreview` can contain a `url` if uploaded.
    await onSubmit(input, files);
    // Clear files after submit if needed by parent
    // files.forEach(file => removeFile(file)); // This would clear files, parent might do it
    if (inputRef.current) {
      inputRef.current.focus();
    }
  };

  const {
    isListening,
    transcript,
    startListening,
    stopListening,
    isTranscribing,
    error: voiceError,
  } = useVoiceInput({
    onTranscript: (text) => onInputChange(text), // Update input with transcript
    onError: (err) => toast.error(err),
  });


  return (
    <div
      className={cn(
        "relative flex w-full flex-col gap-3 overflow-hidden rounded-xl border bg-card p-3 shadow-lg focus-within:ring-2 focus-within:ring-ring dark:bg-card/60",
        isFocused && "ring-2 ring-ring",
        className
      )}
    >
      {/* Files Preview */}
      {files.length > 0 && (
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4">
          {files.map((file) => (
            <div key={file.id || file.preview} className="relative group aspect-square">
              {file.type.startsWith("image/") ? (
                <Image
                  src={file.preview}
                  alt={file.name}
                  fill
                  className="rounded-md object-cover"
                />
              ) : (
                <div className="flex h-full flex-col items-center justify-center rounded-md border bg-muted/30 p-2">
                  <Paperclip className="mb-1 size-6 text-muted-foreground" />
                  <p className="max-w-full truncate text-xs text-muted-foreground">
                    {file.name}
                  </p>
                </div>
              )}
              <Button
                variant="destructive"
                size="icon"
                className="absolute -right-2 -top-2 z-10 size-6 rounded-full opacity-0 shadow-md transition-opacity group-hover:opacity-100"
                onClick={() => handleRemoveFile(file)}
                aria-label={`Remove ${file.name}`}
              >
                <X className="size-3.5" />
              </Button>
            </div>
          ))}
        </div>
      )}

      <form
        ref={formRef}
        onSubmit={(e) => {
          e.preventDefault();
          handleSubmit();
        }}
        className="flex w-full items-end gap-3"
      >
        {/* File Upload Button */}
        <label
          htmlFor="file-upload"
          className="flex cursor-pointer items-center justify-center self-end rounded-md p-2 text-muted-foreground transition-colors duration-200 hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <Paperclip className="size-5" />
          <input
            id="file-upload"
            type="file"
            multiple
            accept="image/*,application/pdf,text/*,.md,.json" // Adjust as needed
            className="sr-only"
            onChange={handleFileChange}
            disabled={files.length >= MAX_FILES || isLoading}
          />
        </label>

        {/* Text Input */}
        <Textarea
          ref={inputRef}
          tabIndex={0}
          onKeyDown={onKeyDown}
          onChange={(e) => onInputChange(e)}
          onFocus={() => setIsFocused(true)}
          onBlur={() => setIsFocused(false)}
          placeholder="Send a message..."
          value={input}
          rows={1}
          maxRows={10}
          spellCheck={false}
          className="min-h-[40px] w-full resize-none scroll-p-2 border-none bg-transparent px-0 py-2 leading-relaxed shadow-none focus-visible:ring-0"
          disabled={isLoading || isListening || isTranscribing}
        />

        {/* Voice Input Button */}
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="shrink-0 text-muted-foreground transition-colors duration-200 hover:text-primary focus-visible:ring-2 focus-visible:ring-ring"
          onClick={isListening ? stopListening : startListening}
          disabled={isLoading || isTranscribing}
          aria-label={isListening ? "Stop listening" : "Start voice input"}
        >
          {isListening ? (
            <Mic className="size-5 text-red-500" />
          ) : isTranscribing ? (
            <Loader2 className="size-5 animate-spin" />
          ) : (
            <Mic className="size-5" />
          )}
        </Button>
        {voiceError && <p className="text-xs text-red-500">{voiceError}</p>}


        {/* Submit Button */}
        <Button
          type="submit"
          size="icon"
          className="shrink-0 bg-primary text-primary-foreground shadow-md transition-all duration-200 hover:bg-primary/90 focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
          disabled={isLoading || (!input.trim() && files.length === 0)}
          aria-label="Send message"
        >
          {isLoading ? (
            <Loader2 className="size-5 animate-spin" />
          ) : (
            <SendHorizontal className="size-5" />
          )}
        </Button>
      </form>
    </div>
  );
};
