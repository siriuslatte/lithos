import Link from 'next/link';
import React, { ReactNode } from 'react';
import { clsx } from 'clsx';
import { ArrowRight } from 'react-feather';

interface FeatureProps {
  children: ReactNode;
  reverse?: boolean;
}

export function Feature({ children, reverse }: FeatureProps) {
  return (
    <div
      className={clsx(
        'relative overflow-hidden rounded-2xl p-6 sm:p-12',
        'flex flex-col xl:flex-row gap-8 xl:gap-20 mb-10',
        'lithos-glass',
        { 'xl:flex-row-reverse': reverse }
      )}
    >
      <span
        className="pointer-events-none absolute inset-x-0 top-0 h-px"
        style={{
          background:
            'linear-gradient(90deg, transparent 0%, var(--lithos-accent) 50%, transparent 100%)',
          opacity: 0.45,
        }}
      />
      <span
        className={clsx(
          'pointer-events-none absolute -z-0 w-[520px] h-[520px] rounded-full blur-3xl opacity-40',
          reverse ? '-left-40 -bottom-40' : '-right-40 -bottom-40'
        )}
        style={{
          background:
            'radial-gradient(circle, var(--lithos-glow) 0%, transparent 70%)',
        }}
      />
      {children}
    </div>
  );
}

interface FeatureContentProps {
  children: ReactNode;
}

export function FeatureContent({ children }: FeatureContentProps) {
  return (
    <div className="relative z-10 flex-1 flex flex-col justify-center gap-3 xl:gap-5">
      {children}
    </div>
  );
}

export function FeatureTitle({ children }: FeatureContentProps) {
  return (
    <h2 className="text-3xl sm:text-4xl font-bold tracking-tight">
      {children}
    </h2>
  );
}

export function FeatureDescription({ children }: FeatureContentProps) {
  return (
    <p
      className="text-base sm:text-lg leading-relaxed"
      style={{ color: 'var(--lithos-muted)' }}
    >
      {children}
    </p>
  );
}

interface FeatureActionProps {
  children: ReactNode;
  href: string;
}

export function FeatureAction({ children, href }: FeatureActionProps) {
  return (
    <Link
      href={href}
      className="group inline-flex items-center gap-1.5 w-fit text-sm font-semibold transition-colors"
      style={{ color: 'var(--lithos-accent)' }}
    >
      {children}
      <ArrowRight
        size={15}
        strokeWidth={2.5}
        className="transition-transform group-hover:translate-x-0.5"
      />
    </Link>
  );
}

interface FeatureDisplayProps {
  children: ReactNode;
  title: ReactNode;
}

export function FeatureDisplay({ children, title }: FeatureDisplayProps) {
  const mappedChildren = React.Children.map(children, (child) => {
    return React.isValidElement(child)
      ? React.cloneElement(child, { className: 'py-3 overflow-x-auto' } as any)
      : null;
  });

  return (
    <div
      className="relative z-10 flex-1 rounded-xl overflow-hidden"
      style={{
        border: '1px solid var(--lithos-border)',
        background:
          'color-mix(in srgb, var(--lithos-bg-elev) 92%, transparent)',
      }}
    >
      <div
        className="flex items-center gap-2 px-4 py-3 text-sm font-medium"
        style={{
          borderBottom: '1px solid var(--lithos-border)',
          color: 'var(--lithos-muted)',
        }}
      >
        <span className="flex gap-1.5">
          <span
            className="block w-2.5 h-2.5 rounded-full"
            style={{ background: 'var(--lithos-border)' }}
          />
          <span
            className="block w-2.5 h-2.5 rounded-full"
            style={{ background: 'var(--lithos-border)' }}
          />
          <span
            className="block w-2.5 h-2.5 rounded-full"
            style={{ background: 'var(--lithos-border)' }}
          />
        </span>
        <span className="ml-2 truncate">{title}</span>
      </div>
      {mappedChildren}
    </div>
  );
}
