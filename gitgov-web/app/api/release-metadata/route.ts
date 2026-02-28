import { NextResponse } from 'next/server';
import { getReleaseMetadata } from '@/lib/release';

export async function GET() {
    const metadata = await getReleaseMetadata();
    return NextResponse.json(metadata);
}
