import React from 'react';

interface TextareaProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
    label: string;
    error?: string;
}

export function Textarea({ label, error, id, className = '', ...props }: TextareaProps) {
    const textareaId = id || label.toLowerCase().replace(/\s+/g, '-');
    return (
        <div className="space-y-1.5">
            <label
                htmlFor={textareaId}
                className="block text-sm font-medium text-gray-300"
            >
                {label}
            </label>
            <textarea
                id={textareaId}
                className={`
          w-full px-4 py-3 rounded-xl
          bg-white/5 border border-white/10
          text-white placeholder-gray-500
          transition-all duration-300 resize-y min-h-[120px]
          focus:border-brand-500/50 focus:bg-white/[0.07] focus:outline-none focus:ring-1 focus:ring-brand-500/30
          ${error ? 'border-red-500/50' : ''}
          ${className}
        `}
                {...props}
            />
            {error && <p className="text-sm text-red-400">{error}</p>}
        </div>
    );
}
