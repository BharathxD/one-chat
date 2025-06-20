"use client";

import { SidebarProvider } from "@workspace/ui/components/sidebar";
import { Toaster } from "@workspace/ui/components/sonner";
import { TooltipProvider } from "@workspace/ui/components/tooltip";
import ThemeProvider from "./theme-provider";
import { TopLoader } from "./top-loader";
import { TRPCProvider } from "./trpc-provider";

export const Providers = ({ children }: React.PropsWithChildren) => (
  <ThemeProvider>
    <TopLoader />
    <TRPCProvider>
      <TooltipProvider delayDuration={0}>
        <SidebarProvider>
          {children}
          <Toaster />
        </SidebarProvider>
      </TooltipProvider>
    </TRPCProvider>
  </ThemeProvider>
);
