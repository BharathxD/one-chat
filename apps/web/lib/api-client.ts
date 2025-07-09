import { env } from "@/env"; // Ensure env is correctly imported if using T3 Env

const AXUM_API_URL = env.NEXT_PUBLIC_AXUM_API_URL;

// Function to retrieve the auth token (e.g., from localStorage)
// This needs to be adapted based on how `better-auth` stores the token.
const getAuthToken = (): string | null => {
  if (typeof window === "undefined") {
    return null; // Cannot access localStorage on server-side
  }
  return localStorage.getItem("authToken"); // Placeholder key
};

interface ApiErrorData {
  message?: string;
  details?: unknown; // Or a more specific error structure if your Axum API has one
}

export class ApiError extends Error {
  status: number;
  data?: ApiErrorData;

  constructor(message: string, status: number, data?: Api_ErrorData) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.data = data;
    Object.setPrototypeOf(this, ApiError.prototype);
  }
}

interface RequestOptions extends RequestInit {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  params?: Record<string, any>; // For URL query parameters
}

async function request<T>(
  endpoint: string,
  options: RequestOptions = {}
): Promise<T> {
  const { params, ...fetchOptions } = options;
  let url = `${AXUM_API_URL}${endpoint}`;

  if (params) {
    const queryParams = new URLSearchParams();
    for (const [key, value] of Object.entries(params)) {
      if (value !== undefined && value !== null) {
        if (Array.isArray(value)) {
          value.forEach((v) => queryParams.append(key, String(v)));
        } else {
          queryParams.append(key, String(value));
        }
      }
    }
    if (queryParams.toString()) {
      url += `?${queryParams.toString()}`;
    }
  }

  const headers = new Headers(fetchOptions.headers || {});
  const token = getAuthToken();
  if (token) {
    headers.append("Authorization", `Bearer ${token}`);
  }

  if (fetchOptions.body && typeof fetchOptions.body === "object" && !(fetchOptions.body instanceof FormData)) {
    if (!headers.has("Content-Type")) {
      headers.append("Content-Type", "application/json");
    }
    fetchOptions.body = JSON.stringify(fetchOptions.body);
  }

  const response = await fetch(url, {
    ...fetchOptions,
    headers,
  });

  if (!response.ok) {
    let errorData: ApiErrorData | undefined;
    try {
      errorData = await response.json();
    } catch (e) {
      // Ignore if response is not JSON
    }
    const errorMessage = errorData?.message || response.statusText || `API request failed with status ${response.status}`;
    throw new ApiError(errorMessage, response.status, errorData);
  }

  // Handle cases where response might be empty (e.g., 204 No Content)
  const contentType = response.headers.get("content-type");
  if (contentType && contentType.includes("application/json")) {
    return response.json() as Promise<T>;
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return undefined as any as T; // Or handle as Promise<void> if that's more appropriate
}

export const apiClient = {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  get: <T>(endpoint: string, params?: Record<string, any>, options?: Omit<RequestOptions, 'params' | 'method' | 'body'>) =>
    request<T>(endpoint, { ...options, method: "GET", params }),

  post: <T, B = unknown>(endpoint: string, body?: B, options?: Omit<RequestOptions, 'method' | 'body'>) =>
    request<T>(endpoint, { ...options, method: "POST", body: body as BodyInit | null | undefined }),

  put: <T, B = unknown>(endpoint: string, body?: B, options?: Omit<RequestOptions, 'method' | 'body'>) =>
    request<T>(endpoint, { ...options, method: "PUT", body: body as BodyInit | null | undefined }),

  delete: <T>(endpoint: string, options?: Omit<RequestOptions, 'method'>) =>
    request<T>(endpoint, { ...options, method: "DELETE" }),

  patch: <T, B = unknown>(endpoint: string, body?: B, options?: Omit<RequestOptions, 'method' | 'body'>) =>
    request<T>(endpoint, { ...options, method: "PATCH", body: body as BodyInit | null | undefined }),
};

// Example Usage:
// apiClient.get<User[]>('/users');
// apiClient.post<Thread, { title: string }>('/threads', { title: 'New Thread' });

// Note: The actual token storage/retrieval mechanism (`getAuthToken`)
// needs to be verified and potentially adjusted based on how `better-auth` works.
// If `better-auth` uses HttpOnly cookies for session management and the Next.js app
// proxies requests to the Axum API (less likely given the separate API goal),
// then token management might be different.
// For a separate API, JWT in localStorage or passed from server components is common.
