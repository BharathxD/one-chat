"use client";

import { env } from "@/env";
import { useQuery } from "@tanstack/react-query"; // Changed from trpc
import { apiClient } from "@/lib/api-client"; // Using new apiClient
import { Button } from "@workspace/ui/components/button";
import { RefreshCcw } from "lucide-react";

// Define the expected shape of the health response from Axum API
interface HealthStatus {
  status: string;
  database: string;
}

// API call function
const fetchHealth = async (): Promise<HealthStatus> => {
  return apiClient.get<HealthStatus>("/health"); // Axum API endpoint
};

const HealthPage = () => {
  const {
    data: health,
    isLoading,
    error,
    refetch,
  } = useQuery<HealthStatus, Error>({ // Specify types for useQuery
    queryKey: ["healthCheck"], // React Query key
    queryFn: fetchHealth,
    refetchOnWindowFocus: true,
    refetchOnMount: true,
    refetchInterval: 10000, // 10s
  });

  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-4 p-4">
      <div className="rounded-lg border bg-card p-6 text-card-foreground shadow-sm">
        <div className="flex flex-col space-y-1.5 text-center">
          <h3 className="whitespace-nowrap font-semibold tracking-tight text-2xl">
            Health Check
          </h3>
          <p className="text-muted-foreground text-sm">
            Current application status.
          </p>
        </div>
        <div className="mt-4 space-y-2 text-sm">
          <div className="flex justify-between">
            <span className="font-medium">Application URL:</span>
            <span className="text-muted-foreground">
              {env.NEXT_PUBLIC_APP_URL}
            </span>
          </div>
          {isLoading && (
            <>
              <div className="flex justify-between">
                <span className="font-medium">API Status:</span>
                <span className="text-muted-foreground">Loading...</span>
              </div>
              <div className="flex justify-between">
                <span className="font-medium">Database:</span>
                <span className="text-muted-foreground">Loading...</span>
              </div>
            </>
          )}
          {error && (
            <>
              <div className="flex justify-between">
                <span className="font-medium">API Status:</span>
                <span className="text-destructive">Error</span>
              </div>
              <div className="flex justify-between">
                <span className="font-medium">Error Details:</span>
                <span className="text-destructive max-w-xs truncate">
                  {error.message}
                </span>
              </div>
            </>
          )}
          {health && (
            <>
              <div className="flex justify-between">
                <span className="font-medium">API Status:</span>
                <span
                  className={
                    health.status === "ok"
                      ? "text-green-500"
                      : "text-destructive"
                  }
                >
                  {health.status}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="font-medium">Database:</span>
                <span
                  className={
                    health.database === "connected"
                      ? "text-green-500"
                      : "text-destructive"
                  }
                >
                  {health.database}
                </span>
              </div>
            </>
          )}
        </div>
        <div className="mt-6 flex items-center justify-center">
          <Button
            variant="outline"
            size="sm"
            onClick={() => refetch()}
            disabled={isLoading}
            className="gap-2"
          >
            <RefreshCcw
              className={isLoading ? "size-4 animate-spin" : "size-4"}
            />
            Refresh
          </Button>
        </div>
      </div>
    </div>
  );
};

export default HealthPage;
