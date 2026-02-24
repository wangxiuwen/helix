/**
 * 阿里云 ECS 服务
 * 通过 HMAC-SHA1 签名直接调用阿里云 OpenAPI
 */

import { useDevOpsStore } from '../stores/useDevOpsStore';

// ========== 签名工具 ==========

async function hmacSha1(key: string, message: string): Promise<string> {
    const encoder = new TextEncoder();
    const keyData = encoder.encode(key);
    const msgData = encoder.encode(message);

    const cryptoKey = await crypto.subtle.importKey(
        'raw', keyData, { name: 'HMAC', hash: 'SHA-1' }, false, ['sign']
    );
    const signature = await crypto.subtle.sign('HMAC', cryptoKey, msgData);
    return btoa(String.fromCharCode(...new Uint8Array(signature)));
}

function percentEncode(str: string): string {
    return encodeURIComponent(str)
        .replace(/!/g, '%21')
        .replace(/'/g, '%27')
        .replace(/\(/g, '%28')
        .replace(/\)/g, '%29')
        .replace(/\*/g, '%2A');
}

function formatDate(): string {
    return new Date().toISOString().replace(/\.\d{3}Z$/, 'Z');
}

function generateNonce(): string {
    return Math.random().toString(36).slice(2) + Date.now().toString(36);
}

// ========== API 签名 ==========

async function signRequest(
    accessKeyId: string,
    accessKeySecret: string,
    params: Record<string, string>
): Promise<string> {
    const allParams: Record<string, string> = {
        Format: 'JSON',
        Version: '2014-05-26',
        AccessKeyId: accessKeyId,
        SignatureMethod: 'HMAC-SHA1',
        Timestamp: formatDate(),
        SignatureVersion: '1.0',
        SignatureNonce: generateNonce(),
        ...params,
    };

    // Sort parameters
    const sortedKeys = Object.keys(allParams).sort();
    const canonicalQuery = sortedKeys
        .map(k => `${percentEncode(k)}=${percentEncode(allParams[k])}`)
        .join('&');

    // Build string to sign
    const stringToSign = `GET&${percentEncode('/')}&${percentEncode(canonicalQuery)}`;

    // Sign
    const signature = await hmacSha1(accessKeySecret + '&', stringToSign);
    allParams['Signature'] = signature;

    // Build URL
    const queryString = Object.entries(allParams)
        .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(v)}`)
        .join('&');

    return queryString;
}

// ========== ECS 接口 ==========

export interface ECSInstance {
    InstanceId: string;
    InstanceName: string;
    Status: string;       // Running | Stopped | Starting | Stopping
    RegionId: string;
    PublicIpAddress: string[];
    InnerIpAddress: string[];
    Cpu: number;
    Memory: number;       // MB
    OSName: string;
    CreationTime: string;
    ExpiredTime: string;
    InstanceType: string;
}

function getAliyunConfig() {
    const state = useDevOpsStore.getState();
    const config = state.cloudConfig?.aliyun;
    if (!config?.accessKeyId || !config?.accessKeySecret) {
        throw new Error('请先在设置中配置阿里云 AccessKey');
    }
    return config;
}

async function callEcsApi(params: Record<string, string>): Promise<any> {
    const config = getAliyunConfig();
    const region = config.region || 'cn-beijing';
    const queryString = await signRequest(
        config.accessKeyId,
        config.accessKeySecret,
        { RegionId: region, ...params }
    );
    const url = `https://ecs.${region}.aliyuncs.com/?${queryString}`;
    const res = await fetch(url);
    const data = await res.json();
    if (data.Code) {
        throw new Error(`阿里云 API 错误: ${data.Code} - ${data.Message}`);
    }
    return data;
}

export async function describeInstances(region?: string): Promise<ECSInstance[]> {
    const params: Record<string, string> = {
        Action: 'DescribeInstances',
        PageSize: '50',
    };
    if (region) params.RegionId = region;
    const data = await callEcsApi(params);
    const instances = data.Instances?.Instance || [];
    return instances.map((i: any) => ({
        InstanceId: i.InstanceId,
        InstanceName: i.InstanceName || i.InstanceId,
        Status: i.Status,
        RegionId: i.RegionId,
        PublicIpAddress: i.PublicIpAddress?.IpAddress || [],
        InnerIpAddress: i.InnerIpAddress?.IpAddress || [],
        Cpu: i.Cpu,
        Memory: i.Memory,
        OSName: i.OSName || '',
        CreationTime: i.CreationTime,
        ExpiredTime: i.ExpiredTime,
        InstanceType: i.InstanceType,
    }));
}

export async function startInstance(instanceId: string): Promise<string> {
    await callEcsApi({ Action: 'StartInstance', InstanceId: instanceId });
    return `实例 ${instanceId} 启动指令已发送`;
}

export async function stopInstance(instanceId: string): Promise<string> {
    await callEcsApi({ Action: 'StopInstance', InstanceId: instanceId });
    return `实例 ${instanceId} 停止指令已发送`;
}

export async function rebootInstance(instanceId: string): Promise<string> {
    await callEcsApi({ Action: 'RebootInstance', InstanceId: instanceId });
    return `实例 ${instanceId} 重启指令已发送`;
}

export async function describeRegions(): Promise<Array<{ RegionId: string; LocalName: string }>> {
    const config = getAliyunConfig();
    const queryString = await signRequest(
        config.accessKeyId,
        config.accessKeySecret,
        { Action: 'DescribeRegions' }
    );
    const url = `https://ecs.aliyuncs.com/?${queryString}`;
    const res = await fetch(url);
    const data = await res.json();
    return data.Regions?.Region || [];
}

// Connection test
export async function testConnection(): Promise<boolean> {
    try {
        await describeRegions();
        return true;
    } catch {
        return false;
    }
}
