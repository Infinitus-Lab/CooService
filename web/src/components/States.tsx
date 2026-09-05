export function Loading() {
  return (
    <div className="flex justify-center py-10">
      <span className="loading loading-spinner loading-md" />
    </div>
  );
}

export function Empty({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex flex-col items-center gap-1 py-10 text-sm opacity-60">
      <span>{children}</span>
    </div>
  );
}

export function ErrorAlert({ message }: { message: string }) {
  return (
    <div role="alert" className="alert alert-error alert-soft mb-4">
      <span>{message}</span>
    </div>
  );
}

export function Field({
  label,
  required,
  children,
}: {
  label: string;
  required?: boolean;
  children: React.ReactNode;
}) {
  return (
    <label className="form-control w-full">
      <div className="label py-1">
        <span className="label-text">
          {label}
          {required && <span className="text-error"> *</span>}
        </span>
      </div>
      {children}
    </label>
  );
}
