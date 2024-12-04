import requests
import os
import time
from statistics import mean, median

from kafka import KafkaConsumer
import json
import datetime
import re

import asyncio
import aiohttp
from asyncio import Semaphore

try:
    print("Configuration:")
    iterations = int(os.getenv('API_ITERATIONS', '1'))
    print(f"API Iterations: {iterations}")

    host = os.getenv('API_HOST', 'localhost')
    port = os.getenv('API_PORT', '8090')
    print(f"API Host: {host}")
    print(f"API Port: {port}")

    kafka_host = os.getenv('KAFKA_HOST', 'localhost:9092')
    kafka_topic = os.getenv('KAFKA_TOPIC', 'message_updates')
    print(f"Kafka Host: {kafka_host}")
    print(f"Kafka Topic: {kafka_topic}")

    max_concurrent = int(os.getenv('API_MAX_CONCURRENT', '10'))
    print(f"Max Concurrent Requests: {max_concurrent}")
except ValueError:
    print("ERROR: API_ITERATIONS environment variable must be a valid integer")
    exit(1)

url = f"http://{host}:{port}/initiate"
headers = { 'Content-Type': 'application/xml' }

consumer = KafkaConsumer(
    kafka_topic,
    bootstrap_servers=[kafka_host],
    group_id='benchmark-consumer',
    auto_offset_reset='latest',
    enable_auto_commit=True,
    value_deserializer=lambda x: json.loads(x.decode('utf-8'))
)

try:
    with open('/app/sample-payload.xml', 'r') as file:
        payload = file.read()
except FileNotFoundError:
    print("ERROR: sample-payload.xml not found")
    exit(1)

response_times = []
successful_calls = 0

async def make_request(session: aiohttp.ClientSession, url: str, headers: dict, 
                      payload: str, i: int, sem: Semaphore) -> float:
    async with sem:  # Control concurrency
        start_time = time.time()
        try:
            async with session.post(url, headers=headers, data=payload) as response:
                await response.text()
                elapsed_time = (time.time() - start_time) * 1000
                if response.status == 200:
                    return True, elapsed_time
                return False, elapsed_time
        except Exception as e:
            print(f"Request {i+1}/{iterations} failed: {str(e)}")
            return False, 0
        
async def run_benchmark(url: str, headers: dict, payload: str, iterations: int):
    sem = Semaphore(max_concurrent)
    connector = aiohttp.TCPConnector(limit=max_concurrent)
    start_time = time.time()
    async with aiohttp.ClientSession(connector=connector) as session:
        tasks = [
            make_request(session, url, headers, payload, i, sem)
            for i in range(iterations)
        ]
        results = await asyncio.gather(*tasks)
        elapsed_time = (time.time() - start_time)

        successful_calls = sum(1 for success, _ in results if success)
        response_times = [time for success, time in results if success]
        
        return successful_calls, response_times, elapsed_time

print(f"Starting benchmark with {iterations} iterations...", flush=True)
successful_calls, response_times, elapsed_time = asyncio.run(
    run_benchmark(url, headers, payload, iterations)
)

print("\nBenchmark Summary:")
print(f"Total Requests: {iterations}")
print(f"Successful Requests: {successful_calls}")
print(f"Failed Requests: {iterations - successful_calls}")
print(f"Average Response Time: {mean(response_times):.2f}ms")
print(f"Median Response Time: {median(response_times):.2f}ms")
print(f"Min Response Time: {min(response_times):.2f}ms")
print(f"Max Response Time: {max(response_times):.2f}ms", flush=True)
print(f"Input TPS: {iterations / elapsed_time:.2f}tps", flush=True)

def clean_str(ts):
    return re.sub("(\d{6})(\d+)", r"\1", ts.replace('+00', '')).replace("Z", "+00:00")

def get_time_from_auditlog(audit_logs):
    if not audit_logs or len(audit_logs) < 1:
        print("No audit logs found", flush=True)
        return None
        
    try:
        first_log = audit_logs[0]
        last_log = audit_logs[-1]
        
        start_time = datetime.datetime.fromisoformat(clean_str(first_log['start_time']))
        # print(f"Parsed start time: {start_time}", flush=True)

        finish_time = datetime.datetime.fromisoformat(clean_str(last_log['finish_time']))
        # print(f"Parsed finish time: {finish_time}", flush=True)
        
        return (start_time, finish_time)
        
    except Exception as e:
        print(f"Error calculating processing time: {e}", flush=True)
        print(f"Raw start time: {first_log['start_time']}", flush=True)
        print(f"Raw finish time: {last_log['finish_time']}", flush=True)
        return None


# After sending all requests, collect processing times
print("\nCollecting processing times from Kafka...", flush=True)
processing_times = []
message_count = 0
first_message_starttime = None
last_message_finishtime = None

while message_count < successful_calls:
    try:
        messages = consumer.poll(timeout_ms=1000)
        if not messages:
            print("No messages received, waiting...", flush=True)
            continue
            
        for tp, msgs in messages.items():
            for msg in msgs:
                try:
                    audit_logs = msg.value.get('audit', [])
                    (start_time, finish_time) = get_time_from_auditlog(audit_logs)
                    if first_message_starttime is None:
                        first_message_starttime = start_time
                    last_message_finishtime = finish_time
                    processing_time = (finish_time - start_time).total_seconds() * 1000
                    if processing_time:
                        processing_times.append(processing_time)
                        message_count += 1
                except Exception as e:
                    print(f"Error processing message: {e}", flush=True)
                    
    except Exception as e:
        print(f"Error polling Kafka: {e}", flush=True)
        time.sleep(1)

print("\nProcessing Time Statistics:", flush=True)
print(f"Messages Processed: {len(processing_times)}")
print(f"Average Processing Time: {mean(processing_times):.2f}ms")
print(f"Median Processing Time: {median(processing_times):.2f}ms")
print(f"Min Processing Time: {min(processing_times):.2f}ms")
print(f"Max Processing Time: {max(processing_times):.2f}ms", flush=True)
print(f"Output TPS: {len(processing_times) / (last_message_finishtime - first_message_starttime).total_seconds():.2f}tps", flush=True)

consumer.close()