import type { CustomAnnotation, MessageWithMetadata } from "@/types";
import { AISDKError } from "ai";
import type { Model } from "./ai";
import { type ModelConfig, OPENROUTER_MODEL_MAP } from "./ai/config";
import { PROVIDER_CONFIGS } from "./api-keys";

export const generateUUID = (): string =>
  "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === "x" ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });

/**
 * Resolves the AI model for a message, checking direct property first, then annotations fallback.
 */
export const resolveModel = (message: MessageWithMetadata): Model | null => {
  const model =
    message.model ??
    (message.annotations as CustomAnnotation[])?.find(
      (annotation) => annotation.type === "model"
    )?.model;

  const cookieModel = getModelFromCookie();

  return model ?? cookieModel ?? null;
};

/**
 * Sets the model preference cookie (client-side only)
 */
export const setModelCookie = (model: Model): void => {
  setCookie("chat-model", model, {
    maxAge: 30 * 24 * 60 * 60, // 30 days in seconds
    sameSite: "lax",
  });
};

/**
 * Gets the model from cookie (client-side only)
 */
export const getModelFromCookie = (): Model | null => {
  return getCookie("chat-model") as Model | null;
};

/**
 * Resolves the initial model for a chat based on messages and cookie fallback
 * 1. If messages exist, use model from latest assistant message
 * 2. If no messages, use model from cookie
 * 3. If no cookie, use default model (now supports dynamic selection based on API keys)
 */
export const resolveInitialModel = (
  messages: MessageWithMetadata[],
  cookieModel: Model | null,
  defaultModel: Model,
  apiKeys?: {
    openai?: string;
    anthropic?: string;
    google?: string;
    openrouter?: string;
  }
): Model => {
  if (cookieModel) return cookieModel;

  // Check if we have messages with model information
  if (messages.length > 0) {
    // Find the latest assistant message to get the model used
    const lastAssistantMessage = messages
      .filter((msg) => msg.role === "assistant")
      .pop();

    if (lastAssistantMessage) {
      const modelFromMessage = resolveModel(lastAssistantMessage);
      if (modelFromMessage) {
        return modelFromMessage;
      }
    }
  }

  // If API keys are provided, try to get the best available model
  if (apiKeys) {
    try {
      const { getBestAvailableDefaultModel } = require("@/lib/ai/models");
      return getBestAvailableDefaultModel(apiKeys);
    } catch {
      // Fallback to provided default if dynamic selection fails
      return defaultModel;
    }
  }

  return defaultModel;
};

export const getOpenRouterModel = (model: ModelConfig) => {
  if (model.apiProvider === "openrouter") return model.id;

  return (
    OPENROUTER_MODEL_MAP[model.id as keyof typeof OPENROUTER_MODEL_MAP] ??
    model.id
  );
};

/**
 * Cookie Management Utilities
 */

export interface CookieOptions {
  expires?: Date;
  maxAge?: number; // in seconds
  path?: string;
  domain?: string;
  secure?: boolean;
  sameSite?: "strict" | "lax" | "none";
  httpOnly?: boolean;
}

/**
 * Sets a cookie with specified options
 */
export const setCookie = (
  name: string,
  value: string,
  options: CookieOptions = {}
): void => {
  if (typeof document === "undefined") return;

  const {
    expires,
    maxAge,
    path = "/",
    domain,
    secure,
    sameSite = "lax",
    httpOnly,
  } = options;

  let cookieString = `${encodeURIComponent(name)}=${encodeURIComponent(value)}`;

  if (expires) {
    cookieString += `;expires=${expires.toUTCString()}`;
  }

  if (maxAge !== undefined) {
    cookieString += `;max-age=${maxAge}`;
  }

  if (path) {
    cookieString += `;path=${path}`;
  }

  if (domain) {
    cookieString += `;domain=${domain}`;
  }

  if (secure) {
    cookieString += ";secure";
  }

  if (sameSite) {
    cookieString += `;samesite=${sameSite}`;
  }

  if (httpOnly) {
    cookieString += ";httponly";
  }

  // biome-ignore lint/nursery/noDocumentCookie: <explanation>
  document.cookie = cookieString;
};

/**
 * Gets a cookie value by name
 */
export const getCookie = (name: string): string | null => {
  if (typeof document === "undefined") return null;

  const nameEQ = `${encodeURIComponent(name)}=`;
  const cookies = document.cookie.split(";");

  for (let cookie of cookies) {
    cookie = cookie.trim();
    if (cookie.indexOf(nameEQ) === 0) {
      return decodeURIComponent(cookie.substring(nameEQ.length));
    }
  }

  return null;
};

/**
 * Removes a cookie by setting its expiry date to the past
 */
export const removeCookie = (
  name: string,
  options: Pick<CookieOptions, "path" | "domain"> = {}
): void => {
  const { path = "/", domain } = options;

  setCookie(name, "", {
    expires: new Date(0),
    path,
    domain,
  });
};

/**
 * Checks if a cookie exists
 */
export const hasCookie = (name: string): boolean => {
  return getCookie(name) !== null;
};

/**
 * Gets all cookies as an object
 */
export const getAllCookies = (): Record<string, string> => {
  if (typeof document === "undefined") return {};

  const cookies: Record<string, string> = {};
  const cookieArray = document.cookie.split(";");

  for (let cookie of cookieArray) {
    cookie = cookie.trim();
    const [name, ...valueParts] = cookie.split("=");
    if (name && valueParts.length > 0) {
      cookies[decodeURIComponent(name)] = decodeURIComponent(
        valueParts.join("=")
      );
    }
  }

  return cookies;
};

/**
 * Safely handles AISDKError instances and provides meaningful error messages
 * with provider-specific formatting when possible
 */
export const handleAISDKError = (error: unknown): string => {
  try {
    if (error instanceof AISDKError) {
      // Safely get provider configs with fallback
      const providers = PROVIDER_CONFIGS ? Object.keys(PROVIDER_CONFIGS) : [];

      // Safely check for responseBody with proper type guards
      const errorWithBody = error as { responseBody?: unknown };
      const responseBody = errorWithBody.responseBody;

      // Only proceed if responseBody is a string
      if (typeof responseBody === "string" && providers.length > 0) {
        const provider = providers.find((providerName) => {
          try {
            return responseBody.includes(providerName);
          } catch {
            return false;
          }
        });

        if (provider) {
          return `${provider.toUpperCase()}: ${error.message || "Unknown error"}`;
        }
      }

      // Fallback to just the error message
      return error.message || "AI SDK Error occurred";
    }

    return "An error occurred";
  } catch (handlerError) {
    // Log the error for debugging but don't let it bubble up
    console.error("Error in handleAISDKError:", handlerError);
    return "An error occurred";
  }
};
