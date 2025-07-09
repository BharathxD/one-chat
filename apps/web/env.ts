import { createEnv } from "@t3-oss/env-core";
import { z } from "zod";

export const env = createEnv({
  server: {
    DATABASE_URL: z.string().url(),

    UPSTASH_REDIS_REST_URL: z.string().url(),
    UPSTASH_REDIS_REST_TOKEN: z.string().min(1),
    UPSTASH_REDIS_URL: z.string().url(),

    OPENAI_API_KEY: z.string().startsWith("sk-").optional(),

    FIRECRAWL_API_KEY: z.string().startsWith("fc-").optional(),

    GOOGLE_CLIENT_ID: z.string().min(1),
    GOOGLE_CLIENT_SECRET: z.string().min(1),

    VERCEL_BLOB_STORE_ID: z.string().min(1).startsWith("store_"),
    VERCEL_BLOB_STORE_BASE_URL: z.string().url(),
    VERCEL_BLOB_READ_WRITE_TOKEN: z
      .string()
      .min(1)
      .startsWith("vercel_blob_rw_"),

    NODE_ENV: z.enum(["development", "production"]).default("development"),
  },

  /**
   * The prefix that client-side variables must have. This is enforced both at
   * a type-level and at runtime.
   */
  clientPrefix: "NEXT_PUBLIC_",

  client: {
    // TODO: use the .env, but the problem is that it's throwing validation error on the client
    // - Notes: I've logged the process.env.NEXT_PUBLIC_APP_URL, it was defined. I don't what's up with this validation error.
    // - If someone knows how to fix this, please do.
    NEXT_PUBLIC_APP_URL: z.string().url().default("https://www.1chat.tech"),
    NEXT_PUBLIC_AXUM_API_URL: z.string().url().default("http://localhost:3001/api"),
  },

  /**
   * What object holds the environment variables at runtime. This is usually
   * `process.env` or `import.meta.env`.
   */
  runtimeEnv: process.env,

  /**
   * By default, this library will feed the environment variables directly to
   * the Zod validator.
   *
   * This means that if you have an empty string for a value that is supposed
   * to be a number (e.g. `PORT=` in a ".env" file), Zod will incorrectly flag
   * it as a type mismatch violation. Additionally, if you have an empty string
   * for a value that is supposed to be a string with a default value (e.g.
   * `DOMAIN=` in an ".env" file), the default value will never be applied.
   *
   * In order to solve these issues, we recommend that all new projects
   * explicitly specify this option as true.
   */
  emptyStringAsUndefined: true,
});
