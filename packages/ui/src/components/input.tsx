import type * as React from "react";

import { cn } from "@workspace/ui/lib/utils";
import type { LucideIcon } from "lucide-react";

function Input({
  className,
  type,
  icon: Icon,
  shellClassName,
  ...props
}: React.ComponentProps<"input"> & {
  icon?: LucideIcon;
  shellClassName?: string;
}) {
  return (
    <div className={cn("relative", shellClassName)}>
      <input
        type={type}
        data-slot="input"
        className={cn(
          "flex h-9 w-full min-w-0 rounded-md border border-input bg-transparent px-3 py-1 text-base shadow-xs outline-none transition-[color,box-shadow] selection:bg-primary selection:text-primary-foreground file:inline-flex file:h-7 file:border-0 file:bg-transparent file:font-medium file:text-foreground file:text-sm placeholder:text-muted-foreground disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm dark:bg-input/30",
          "focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50",
          "aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40",
          Icon && "pl-10",
          className
        )}
        {...props}
      />
      {Icon && (
        <div className="pointer-events-none absolute inset-y-0 start-0 flex items-center justify-center ps-3 text-muted-foreground/80 peer-disabled:opacity-50">
          <Icon size={16} aria-hidden="true" />
        </div>
      )}
    </div>
  );
}

export { Input };
