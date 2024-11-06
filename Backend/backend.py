from fastapi import FastAPI, UploadFile, File
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse
from io import BytesIO
from PIL import Image
from ultralytics import YOLO
import logging


# Set up basic logging
logging.basicConfig(level=logging.INFO)

# Load your custom YOLOv11 model
model = YOLO("/mnt/c/Users/Nick/Desktop/Dominos/Backend/best.pt", task="detect")

app = FastAPI()

app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://127.0.0.1:8080"],  # List the allowed origins (your frontend URL)
    allow_credentials=True,
    allow_methods=["*"],  # Allow all methods (POST, GET, etc.)
    allow_headers=["*"],  # Allow all headers
)

@app.post("/process-image")
async def process_image(file: UploadFile = File(...)):
    logging.info("Received request to /process-image")
    
    # Read the uploaded file
    contents = await file.read()
    logging.info(f"Uploaded file size: {len(contents)} bytes")

    try:
        image = Image.open(BytesIO(contents))
        logging.info("Image successfully opened")

        # Perform YOLOv11 inference on the image
        logging.info("Running YOLO inference...")
        results = model(image)
        logging.info(f"YOLO inference completed with {len(results)} results")
        # Save the inference image to a BytesIO stream
        output_image = BytesIO()
        results[0].plot()  # Add bounding boxes and labels to the image
        output_image.seek(0)

        logging.info("Processed image saved successfully")
        
        # Return the processed image as a response
        return StreamingResponse(output_image, media_type="image/jpeg")

    except Exception as e:
        logging.error(f"Error processing image: {str(e)}")
        return {"error": str(e)}

# To run the server:
# uvicorn backend:app --reload
