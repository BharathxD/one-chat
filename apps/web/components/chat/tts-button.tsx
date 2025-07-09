"use client";

import { Button } from "@workspace/ui/components/button";
import { toast } from "@workspace/ui/components/sonner";
import { useMutation } from "@tanstack/react-query"; // Changed
import { apiClient, ApiError } from "@/lib/api-client"; // Changed
import { useUserSettings } from "@/hooks/use-user-settings"; // To get API keys
import { Loader2, Play, StopCircle } from "lucide-react";
import React, { useEffect, useRef, useState } from "react";

interface TextToSpeechButtonProps {
  text: string;
  disabled?: boolean;
}

// Define payload and response types matching Axum API
interface TextToSpeechPayload {
  text: string;
  voice?: string;
  model?: string;
  speed?: number;
  apiKey?: string; // User's API key for the selected provider
  provider?: "openai" | "google";
}

interface TextToSpeechResponse {
  audio: string; // base64 encoded audio
  format: string; // e.g., "mp3" or "wav"
  voice: string;
  text_length: number;
}

export const TextToSpeechButton: React.FC<TextToSpeechButtonProps> = ({
  text,
  disabled,
}) => {
  const [isPlaying, setIsPlaying] = useState(false);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const { settings } = useUserSettings(); // For API keys and TTS preferences

  const generateSpeechMutation = useMutation<
    TextToSpeechResponse,
    ApiError,
    TextToSpeechPayload
  >({
    mutationFn: async (payload) => {
      return apiClient.post<TextToSpeechResponse, TextToSpeechPayload>(
        "/voice/tts",
        payload
      );
    },
    onSuccess: (data) => {
      const audioSrc = `data:audio/${data.format};base64,${data.audio}`;
      if (audioRef.current) {
        audioRef.current.src = audioSrc;
        audioRef.current.play().catch(err => {
          toast.error("Failed to play audio automatically.");
          console.error("Audio play error:", err);
          setIsPlaying(false);
        });
        setIsPlaying(true);
      }
    },
    onError: (error) => {
      toast.error(`Speech generation failed: ${error.message}`);
      setIsPlaying(false);
    },
  });

  const handlePlayPause = () => {
    if (isPlaying && audioRef.current) {
      audioRef.current.pause();
      audioRef.current.currentTime = 0; // Reset audio
      setIsPlaying(false);
    } else if (text) {
      const provider = settings.ttsProvider || "openai";
      let apiKey: string | undefined;
      if (provider === "openai") {
        apiKey = settings.openaiApiKey;
      } else if (provider === "google") {
        apiKey = settings.googleApiKey; // Assuming settings structure
      }

      if (!apiKey) {
        toast.error(
          `${
            provider.charAt(0).toUpperCase() + provider.slice(1)
          } API key not set. Please configure it in settings.`
        );
        return;
      }

      generateSpeechMutation.mutate({
        text,
        voice: settings.ttsVoice || (provider === "openai" ? "alloy" : "elevenlabs-alloy"), // Default based on provider
        model: settings.ttsModel || (provider === "openai" ? "gpt-4o-mini-tts" : "gemini-2.5-flash-preview-tts"),
        speed: settings.ttsSpeed || 1.0,
        apiKey,
        provider: provider as "openai" | "google",
      });
    }
  };

  // Cleanup audio element on unmount
  useEffect(() => {
    const currentAudio = audioRef.current;
    return () => {
      if (currentAudio) {
        currentAudio.pause();
        currentAudio.src = "";
      }
    };
  }, []);

  // Listen for audio end to reset playing state
  useEffect(() => {
    if (audioRef.current) {
      const handleAudioEnd = () => setIsPlaying(false);
      audioRef.current.addEventListener("ended", handleAudioEnd);
      return () => {
        audioRef.current?.removeEventListener("ended", handleAudioEnd);
      };
    }
  }, [audioRef.current]);


  return (
    <>
      <Button
        variant="ghost"
        size="icon"
        className="size-7 text-muted-foreground transition-colors hover:text-foreground"
        onClick={handlePlayPause}
        disabled={disabled || generateSpeechMutation.isPending || !text.trim()}
        aria-label={isPlaying ? "Stop speech" : "Play speech"}
      >
        {generateSpeechMutation.isPending ? (
          <Loader2 className="size-3.5 animate-spin" />
        ) : isPlaying ? (
          <StopCircle className="size-3.5" />
        ) : (
          <Play className="size-3.5" />
        )}
      </Button>
      <audio ref={audioRef} className="hidden" />
    </>
  );
};
