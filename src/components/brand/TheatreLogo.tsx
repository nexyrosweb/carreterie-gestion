import logo from "@/assets/brand/logo-theatre-carreterie.jpg";
import icon from "@/assets/brand/icon-tlc.png";
import { cn } from "@/lib/cn";

type Variant = "full" | "mark";

export function TheatreLogo({
  variant = "full",
  className,
  alt = "Théâtre La Carreterie",
}: {
  variant?: Variant;
  className?: string;
  alt?: string;
}) {
  if (variant === "mark") {
    return (
      <img
        src={icon}
        alt={alt}
        className={cn("h-10 w-10 rounded-md object-cover shadow-sm", className)}
        draggable={false}
      />
    );
  }

  return (
    <img
      src={logo}
      alt={alt}
      className={cn(
        "w-full max-w-sm rounded-md object-cover shadow-md ring-1 ring-[var(--color-border)]",
        className,
      )}
      draggable={false}
    />
  );
}
