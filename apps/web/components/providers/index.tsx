"use client";

import { SidebarProvider } from "@workspace/ui/components/sidebar";
import { Toaster } from "@workspace/ui/components/sonner";
import { TooltipProvider } from "@workspace/ui/components/tooltip";
import ThemeProvider from "./theme-provider";
import { TopLoader } from "./top-loader";
// TRPCProvider removed
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"; // Added
import { useState } from "react"; // Added for QueryClient

export const Providers = ({ children }: React.PropsWithChildren) => {
  // Set up QueryClient for react-query
  const [queryClient] = useState(() => new QueryClient());

  return (
    <ThemeProvider>
      <TopLoader />
      <QueryClientProvider client={queryClient}> {/* Added */}
        <TooltipProvider delayDuration={0}>
          <SidebarProvider>
            {children}
            <Toaster />
          </SidebarProvider>
        </TooltipProvider>
      </QueryClientProvider> {/* Added */}
    </ThemeProvider>
  );
};
